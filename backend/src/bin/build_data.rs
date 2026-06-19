//! Data-build pipeline.
//!
//! Fetches street data from the Overpass API (OSM) for a registry of cities,
//! then writes the per-city manifests and global manifests consumed by the
//! frontend under `frontend/public/data/`.
//!
//! A failed/empty Overpass fetch is a hard error by default (the city is
//! skipped) so the build never silently ships fake data. Pass
//! `--allow-synthetic` to fall back to a deterministic synthetic street grid
//! instead — useful for offline CI.
//!
//! Run from the `backend/` directory:
//!   cargo run --release --bin build-data [--allow-synthetic] [CITY_ID ...]
//!
//! To add or update a town: add/edit its entry in `cities.json` (next to
//! this crate's `Cargo.toml`), then run the command above with that town's
//! id (or with no id to rebuild everything).
//!
//! To delete a town: with its entry still in `cities.json`, run
//!   cargo run --release --bin build-data -- --delete CITY_ID
//! to remove its generated manifest directory and cached raw fetches, then
//! remove its entry from `cities.json` and rebuild with no id to refresh the
//! global manifests (cities.json / players.json / leaderboard.json).

use anyhow::{Context, Result};
use chrono::Utc;
use city_challenge_backend::route;
use city_challenge_backend::scoring::{
    calculate_achievement_bonus_points, calculate_total_points, is_all_streets_completed,
    rank_entries, AchievementType, HasPoints,
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const OVERPASS_API_URL: &str = "https://overpass-api.de/api/interpreter";
const NOMINATIM_REVERSE_URL: &str = "https://nominatim.openstreetmap.org/reverse";
/// Nominatim's usage policy caps anonymous traffic at 1 request/second.
const NOMINATIM_MIN_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1100);

/// One entry of the city registry, loaded from `cities.json`. Pass one or
/// more ids as CLI arguments to restrict the build to a subset, e.g.:
///   cargo run --release --bin build-data -- FR-75001-Paris
#[derive(Debug, Clone, Deserialize)]
struct CityConfig {
    /// `{PAYS}-{CODE_POSTAL}-{VILLE}`
    id: String,
    /// ISO 3166-1 alpha-3 country code
    country: String,
    region: String,
    department: String,
    postal_code: String,
    name: String,
    lat: f64,
    lng: f64,
    /// half-width of the bounding box, in degrees
    bbox: f64,
}

/// Load the city registry from `cities.json`, next to this crate's `Cargo.toml`.
fn load_city_registry(path: &Path) -> Result<Vec<CityConfig>> {
    let json = fs::read_to_string(path)
        .with_context(|| format!("failed to read city registry at {}", path.display()))?;
    serde_json::from_str(&json)
        .with_context(|| format!("failed to parse city registry at {}", path.display()))
}

/// Fixed roster of synthetic players used to seed leaderboard data.
/// Deterministic, seeded random results are generated for each player on
/// each city edition so the dataset stays idempotent.
struct SyntheticPlayer {
    id: &'static str,
    name: &'static str,
    /// ISO 3166-1 alpha-3 country code
    country: &'static str,
}

const PLAYERS: &[SyntheticPlayer] = &[
    SyntheticPlayer {
        id: "player-alice",
        name: "Alice Martin",
        country: "FRA",
    },
    SyntheticPlayer {
        id: "player-bob",
        name: "Bob Dupont",
        country: "BEL",
    },
    SyntheticPlayer {
        id: "player-charlie",
        name: "Charlie Smith",
        country: "GBR",
    },
    SyntheticPlayer {
        id: "player-dana",
        name: "Dana Müller",
        country: "DEU",
    },
    SyntheticPlayer {
        id: "player-elena",
        name: "Elena Rossi",
        country: "ITA",
    },
    SyntheticPlayer {
        id: "player-fatima",
        name: "Fatima Garcia",
        country: "ESP",
    },
    SyntheticPlayer {
        id: "player-gus",
        name: "Gus Johnson",
        country: "USA",
    },
    SyntheticPlayer {
        id: "player-hana",
        name: "Hana Visser",
        country: "NLD",
    },
];

/// Tiny deterministic PRNG (mulberry32) so the dataset is reproducible for a
/// given seed string.
fn create_rng(seed: &str) -> impl FnMut() -> f64 {
    let mut h: u32 = 1779033703 ^ (seed.len() as u32);
    for byte in seed.bytes() {
        h = (h ^ byte as u32).wrapping_mul(3432918353);
        h = h.rotate_left(13);
    }
    let mut state = h;
    move || {
        state = state.wrapping_add(0x6d2b79f5);
        let mut t = (state ^ (state >> 15)).wrapping_mul(state | 1);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61)) ^ t;
        f64::from(t ^ (t >> 14)) / 4294967296.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StreetData {
    id: String,
    name: String,
    nodes: Vec<(f64, f64)>,
    length: f64,
    /// OSM node ids aligned 1:1 with `nodes`. Empty for synthetic data.
    /// Used to split ways at shared intersections when building the route
    /// graph (see [`split_into_segments`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    node_ids: Vec<i64>,
}

/// Calculate distance in kilometers along a polyline using the haversine formula.
fn calculate_distance(coords: &[(f64, f64)]) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;

    let mut distance = 0.0;
    for window in coords.windows(2) {
        let (lat1, lng1) = window[0];
        let (lat2, lng2) = window[1];
        let d_lat = (lat2 - lat1).to_radians();
        let d_lng = (lng2 - lng1).to_radians();
        let a = (d_lat / 2.0).sin().powi(2)
            + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lng / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        distance += EARTH_RADIUS_KM * c;
    }
    distance
}

#[derive(Debug, Deserialize)]
struct OverpassResponse {
    #[serde(default)]
    elements: Vec<OverpassElement>,
}

#[derive(Debug, Deserialize)]
struct OverpassElement {
    #[serde(rename = "type")]
    kind: String,
    id: i64,
    #[serde(default)]
    tags: Option<OverpassTags>,
    #[serde(default)]
    geometry: Option<Vec<OverpassNode>>,
    /// OSM node ids for a way, aligned 1:1 with `geometry`. Present because
    /// the query uses `out body geom`.
    #[serde(default)]
    nodes: Option<Vec<i64>>,
    /// Member ways (with their own inlined geometry, since the query uses
    /// `out geom`), present only for `type: "relation"` elements.
    #[serde(default)]
    members: Option<Vec<OverpassMember>>,
}

#[derive(Debug, Deserialize)]
struct OverpassTags {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OverpassNode {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Deserialize)]
struct OverpassMember {
    #[serde(rename = "type")]
    kind: String,
    role: String,
    #[serde(default)]
    geometry: Option<Vec<OverpassNode>>,
}

#[derive(Debug, Default, Deserialize)]
struct NominatimAddress {
    road: Option<String>,
    pedestrian: Option<String>,
    footway: Option<String>,
    cycleway: Option<String>,
    path: Option<String>,
    house_number: Option<String>,
    postcode: Option<String>,
    city: Option<String>,
    town: Option<String>,
    village: Option<String>,
    municipality: Option<String>,
}

impl NominatimAddress {
    /// The street/way name, whatever its OSM highway subtype.
    fn road_name(&self) -> Option<&str> {
        self.road
            .as_deref()
            .or(self.pedestrian.as_deref())
            .or(self.footway.as_deref())
            .or(self.cycleway.as_deref())
            .or(self.path.as_deref())
    }

    fn locality(&self) -> Option<&str> {
        self.city
            .as_deref()
            .or(self.town.as_deref())
            .or(self.village.as_deref())
            .or(self.municipality.as_deref())
    }

    /// Best-effort human-readable address, e.g. "12 Rue de la Paix, 75001 Paris".
    fn format(&self, display_name: Option<&str>) -> Option<String> {
        let street = self
            .house_number
            .as_deref()
            .zip(self.road_name())
            .map(|(number, road)| format!("{number} {road}"))
            .or_else(|| self.road_name().map(str::to_string));

        let parts: Vec<String> = [
            street,
            self.postcode.clone(),
            self.locality().map(str::to_string),
        ]
        .into_iter()
        .flatten()
        .collect();

        if parts.is_empty() {
            display_name.map(str::to_string)
        } else {
            Some(parts.join(", "))
        }
    }
}

#[derive(Debug, Deserialize)]
struct NominatimResponse {
    #[serde(default)]
    address: NominatimAddress,
    display_name: Option<String>,
}

/// Cached result of a reverse-geocode lookup at a given point, persisted to
/// disk so re-running the build never re-queries the same coordinate twice.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GeocodeCacheEntry {
    road: Option<String>,
    address: Option<String>,
}

/// Rate-limited, disk-cached Nominatim reverse-geocoding client. Nominatim's
/// usage policy caps anonymous traffic at 1 request/second, so every network
/// call goes through `throttle` first; calls are sequential by construction
/// (this is `&mut self`), so a single last-call timestamp is enough.
///
/// Lookups are cached by coordinate (rounded to ~1m) across the whole build
/// (all cities) and persisted to `cache_path` via `save`, so re-running
/// `build-data` for the same city/town never pays for the same point twice.
struct Geocoder<'a> {
    client: &'a reqwest::Client,
    last_call: Option<std::time::Instant>,
    cache_path: PathBuf,
    cache: HashMap<String, GeocodeCacheEntry>,
    dirty: bool,
}

impl<'a> Geocoder<'a> {
    fn new(client: &'a reqwest::Client, cache_path: PathBuf) -> Self {
        let cache = fs::read_to_string(&cache_path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Self {
            client,
            last_call: None,
            cache_path,
            cache,
            dirty: false,
        }
    }

    /// Rounds to ~1m precision so nearby points (e.g. two ways' midpoints a
    /// few centimeters apart) share a cache entry.
    fn cache_key(lat: f64, lng: f64) -> String {
        format!("{lat:.5},{lng:.5}")
    }

    async fn throttle(&mut self) {
        if let Some(last) = self.last_call {
            let elapsed = last.elapsed();
            if elapsed < NOMINATIM_MIN_INTERVAL {
                tokio::time::sleep(NOMINATIM_MIN_INTERVAL - elapsed).await;
            }
        }
        self.last_call = Some(std::time::Instant::now());
    }

    /// Reverse-geocode a point. Returns `None` on any failure (network,
    /// non-2xx, malformed body) so a flaky Nominatim response never blocks
    /// the rest of the build.
    async fn fetch(&mut self, lat: f64, lng: f64) -> Option<NominatimResponse> {
        self.throttle().await;
        self.client
            .get(NOMINATIM_REVERSE_URL)
            .query(&[
                ("format", "jsonv2"),
                ("lat", lat.to_string().as_str()),
                ("lon", lng.to_string().as_str()),
                ("zoom", "18"),
                ("addressdetails", "1"),
            ])
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json::<NominatimResponse>()
            .await
            .ok()
    }

    /// Reverse-geocode a point, hitting the on-disk cache first. A single
    /// network call (when uncached) resolves both the road name and the
    /// formatted address at once, since both are derived from the same
    /// Nominatim response.
    async fn lookup(&mut self, lat: f64, lng: f64) -> GeocodeCacheEntry {
        let key = Self::cache_key(lat, lng);
        if let Some(entry) = self.cache.get(&key) {
            return entry.clone();
        }

        let response = self.fetch(lat, lng).await;
        let entry = GeocodeCacheEntry {
            road: response
                .as_ref()
                .and_then(|r| r.address.road_name().map(str::to_string)),
            address: response
                .as_ref()
                .and_then(|r| r.address.format(r.display_name.as_deref())),
        };
        self.cache.insert(key, entry.clone());
        self.dirty = true;
        entry
    }

    /// Reverse-geocode a point's road name, for naming unnamed OSM ways.
    async fn road_name(&mut self, lat: f64, lng: f64) -> Option<String> {
        self.lookup(lat, lng).await.road
    }

    /// Reverse-geocode a point into a formatted address string.
    async fn reverse_address(&mut self, lat: f64, lng: f64) -> Option<String> {
        self.lookup(lat, lng).await.address
    }

    /// Persists the cache to disk. Cheap no-op if nothing changed.
    fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.cache_path, serde_json::to_string_pretty(&self.cache)?)?;
        self.dirty = false;
        Ok(())
    }
}

/// A commune's administrative boundary, as one or more closed outer rings
/// (lat, lng), used to clip fetched streets so the route never wanders into
/// a neighboring commune.
struct Boundary {
    rings: Vec<Vec<(f64, f64)>>,
}

/// Fetch a city's administrative boundary polygon (OSM `boundary=administrative`)
/// from Overpass. Matches both `admin_level=8` (the commune level in France)
/// and `admin_level=9` (arrondissements, used by Paris/Lyon/Marseille), since
/// a `cities.json` entry like "Paris" (postal code 75001) means the 1st
/// arrondissement, not the whole city.
///
/// Tries a narrow query first (name substring + exact postal code match,
/// e.g. matches "Paris 1er Arrondissement" by its `postal_code=75001` tag
/// without ever fetching the whole city's geometry). Many relations (e.g.
/// Lyon's arrondissements) carry no `postal_code` tag at all, so when the
/// narrow query comes back empty this falls back to a broad name-only query
/// across every same-named place worldwide, disambiguated by picking
/// whichever candidate's polygon actually contains the configured (lat,
/// lng) and, among those, the smallest one (the most specific admin level —
/// an arrondissement over the city that contains it, rather than whichever
/// has the closest average node position, which favors large polygons).
///
/// Returns `Ok(None)` (not an error) when no relation is found, so callers
/// can fall back to the unclipped bbox fetch.
async fn fetch_city_boundary(
    client: &reqwest::Client,
    city: &CityConfig,
) -> Result<Option<Boundary>> {
    let escaped_name = city.name.replace('\\', "\\\\").replace('"', "\\\"");
    let escaped_postal = city.postal_code.replace('\\', "\\\\").replace('"', "\\\"");

    let narrow_query = format!(
        "[out:json][timeout:60];\n\
        relation[\"boundary\"=\"administrative\"][\"name\"~\"{escaped_name}\"]\
        [\"postal_code\"~\"(^|;){escaped_postal}(;|$)\"];\n\
        out geom;"
    );
    let relations = run_overpass_query(client, &narrow_query).await?;
    if let Some(boundary) = pick_boundary(&relations, city) {
        return Ok(Some(boundary));
    }

    let broad_query = format!(
        "[out:json][timeout:90];\n\
        relation[\"boundary\"=\"administrative\"][\"admin_level\"~\"^(8|9)$\"][\"name\"~\"{escaped_name}\"];\n\
        out geom;"
    );
    let relations = run_overpass_query(client, &broad_query).await?;
    Ok(pick_boundary(&relations, city))
}

/// POST a query to the Overpass API and parse the response, retrying on
/// transient failures. overpass-api.de load-balances across several
/// backends that occasionally return a transient error (HTTP 406 without a
/// User-Agent, 429 rate-limited, 504 under load); retrying a handful of
/// times with a growing backoff works around this.
async fn run_overpass_query(client: &reqwest::Client, query: &str) -> Result<OverpassResponse> {
    const MAX_ATTEMPTS: u32 = 4;
    let mut last_error = None;

    for attempt in 1..=MAX_ATTEMPTS {
        let result = async {
            let response = client
                .post(OVERPASS_API_URL)
                .form(&[("data", query)])
                .send()
                .await?
                .error_for_status()?;
            response.json::<OverpassResponse>().await
        }
        .await;

        match result {
            Ok(response) => return Ok(response),
            Err(err) => {
                println!("  Overpass attempt {attempt}/{MAX_ATTEMPTS} failed: {err}");
                last_error = Some(err);
                if attempt < MAX_ATTEMPTS {
                    tokio::time::sleep(std::time::Duration::from_secs(2 * u64::from(attempt)))
                        .await;
                }
            }
        }
    }

    Err(last_error.expect("at least one attempt was made").into())
}

/// Among `data`'s relations, pick the boundary polygon to use: prefer
/// whichever candidate's own polygon contains the configured (lat, lng),
/// and among those the one with the smallest bounding-box area (the most
/// specific admin level). Falls back to the smallest candidate overall if
/// none contain the point (e.g. floating-point edge case near the border).
fn pick_boundary(data: &OverpassResponse, city: &CityConfig) -> Option<Boundary> {
    let mut scored: Vec<(bool, f64, Boundary)> = data
        .elements
        .iter()
        .filter(|el| el.kind == "relation")
        .filter_map(|relation| {
            let outer_ways: Vec<Vec<(f64, f64)>> = relation
                .members
                .as_ref()?
                .iter()
                .filter(|m| m.kind == "way" && (m.role == "outer" || m.role.is_empty()))
                .filter_map(|m| m.geometry.as_ref())
                .map(|geometry| geometry.iter().map(|n| (n.lat, n.lon)).collect())
                .filter(|nodes: &Vec<(f64, f64)>| nodes.len() >= 2)
                .collect();
            let rings = assemble_rings(outer_ways);
            if rings.is_empty() {
                return None;
            }
            let boundary = Boundary { rings };
            let contains = point_in_boundary((city.lat, city.lng), &boundary);
            let (min_lat, min_lng, max_lat, max_lng) = boundary_bbox(&boundary, 0.0);
            let area = (max_lat - min_lat) * (max_lng - min_lng);
            Some((contains, area, boundary))
        })
        .collect();

    if scored.iter().any(|(contains, ..)| *contains) {
        scored.retain(|(contains, ..)| *contains);
    }
    scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
    scored.into_iter().map(|(_, _, boundary)| boundary).next()
}

/// Stitch member ways (in arbitrary order/direction) into closed ring(s) by
/// chaining shared endpoints. A relation's `outer` members aren't guaranteed
/// to be returned in ring order, so this greedily extends a ring from either
/// end until it closes or nothing else connects.
fn assemble_rings(mut remaining: Vec<Vec<(f64, f64)>>) -> Vec<Vec<(f64, f64)>> {
    const EPS: f64 = 1e-7;
    let pts_eq = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() < EPS;

    let mut rings = Vec::new();
    while !remaining.is_empty() {
        let mut ring = remaining.remove(0);
        loop {
            if ring.len() > 2 && pts_eq(*ring.first().unwrap(), *ring.last().unwrap()) {
                break;
            }
            let (head, tail) = (*ring.first().unwrap(), *ring.last().unwrap());
            let found = remaining.iter().enumerate().find_map(|(i, w)| {
                let (w_first, w_last) = (*w.first().unwrap(), *w.last().unwrap());
                if pts_eq(w_first, tail) {
                    Some((i, false, true))
                } else if pts_eq(w_last, tail) {
                    Some((i, true, true))
                } else if pts_eq(w_last, head) {
                    Some((i, false, false))
                } else if pts_eq(w_first, head) {
                    Some((i, true, false))
                } else {
                    None
                }
            });
            match found {
                Some((i, reverse, append_to_tail)) => {
                    let mut way = remaining.remove(i);
                    if reverse {
                        way.reverse();
                    }
                    if append_to_tail {
                        ring.extend(way.into_iter().skip(1));
                    } else {
                        way.pop();
                        way.extend(ring);
                        ring = way;
                    }
                }
                None => break, // dangling: nothing left connects to either end
            }
        }
        if ring.len() >= 3 {
            if !pts_eq(*ring.first().unwrap(), *ring.last().unwrap()) {
                let first = ring[0];
                ring.push(first);
            }
            rings.push(ring);
        }
    }
    rings
}

/// Point-in-polygon test (ray casting) against a single closed ring.
fn point_in_ring(point: (f64, f64), ring: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (xi, yi) = ring[i];
        let (xj, yj) = ring[j];
        if (yi > point.1) != (yj > point.1) {
            let x_intersect = xi + (point.1 - yi) / (yj - yi) * (xj - xi);
            if point.0 < x_intersect {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

fn point_in_boundary(point: (f64, f64), boundary: &Boundary) -> bool {
    boundary.rings.iter().any(|ring| point_in_ring(point, ring))
}

/// Bounding box `(min_lat, min_lng, max_lat, max_lng)` of every point in
/// `boundary`, padded by `margin_deg` so the Overpass fetch (which is
/// clipped to the boundary afterwards anyway) also picks up streets whose
/// geometry starts just outside the commune but crosses into it.
fn boundary_bbox(boundary: &Boundary, margin_deg: f64) -> (f64, f64, f64, f64) {
    let mut min_lat = f64::INFINITY;
    let mut min_lng = f64::INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    let mut max_lng = f64::NEG_INFINITY;
    for ring in &boundary.rings {
        for &(lat, lng) in ring {
            min_lat = min_lat.min(lat);
            min_lng = min_lng.min(lng);
            max_lat = max_lat.max(lat);
            max_lng = max_lng.max(lng);
        }
    }
    (
        min_lat - margin_deg,
        min_lng - margin_deg,
        max_lat + margin_deg,
        max_lng + margin_deg,
    )
}

/// Intersection of segment `p0`-`p1` with segment `a`-`b`, if any, as
/// `(t, point)` where `t` is the fractional position along `p0`-`p1`.
fn segment_intersection(
    p0: (f64, f64),
    p1: (f64, f64),
    a: (f64, f64),
    b: (f64, f64),
) -> Option<(f64, (f64, f64))> {
    let (x1, y1) = p0;
    let (x2, y2) = p1;
    let (x3, y3) = a;
    let (x4, y4) = b;
    let d = (x2 - x1) * (y4 - y3) - (y2 - y1) * (x4 - x3);
    if d.abs() < 1e-15 {
        return None;
    }
    let t = ((x3 - x1) * (y4 - y3) - (y3 - y1) * (x4 - x3)) / d;
    let s = ((x3 - x1) * (y2 - y1) - (y3 - y1) * (x2 - x1)) / d;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&s) {
        Some((t, (x1 + t * (x2 - x1), y1 + t * (y2 - y1))))
    } else {
        None
    }
}

/// The first point (closest to `p0`) where segment `p0`-`p1` crosses any
/// edge of `boundary`.
fn first_boundary_crossing(
    p0: (f64, f64),
    p1: (f64, f64),
    boundary: &Boundary,
) -> Option<(f64, f64)> {
    boundary
        .rings
        .iter()
        .flat_map(|ring| ring.windows(2))
        .filter_map(|edge| segment_intersection(p0, p1, edge[0], edge[1]))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, point)| point)
}

/// Clip a polyline to the portion(s) lying inside `boundary`, splitting it
/// into one sub-polyline per inside run and inserting the exact crossing
/// point at every entry/exit. This is what makes a street that runs into a
/// neighboring commune stop right at the border instead of continuing past
/// it (the cut end then naturally becomes a dead end the route solver
/// already knows how to turn around at).
fn clip_polyline(nodes: &[(f64, f64)], boundary: &Boundary) -> Vec<Vec<(f64, f64)>> {
    let mut result = Vec::new();
    let mut current: Vec<(f64, f64)> = Vec::new();
    let mut prev_inside = point_in_boundary(nodes[0], boundary);
    if prev_inside {
        current.push(nodes[0]);
    }

    for window in nodes.windows(2) {
        let (p0, p1) = (window[0], window[1]);
        let inside1 = point_in_boundary(p1, boundary);

        if prev_inside == inside1 {
            if inside1 {
                current.push(p1);
            }
        } else if let Some(cross) = first_boundary_crossing(p0, p1, boundary) {
            if prev_inside {
                // Exiting: close the current run at the border.
                current.push(cross);
                if current.len() >= 2 {
                    result.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            } else {
                // Entering: start a new run at the border.
                current = vec![cross, p1];
            }
        } else if inside1 {
            // Numerically failed to locate the crossing (point exactly on
            // the boundary); fall back to starting the run at `p1`.
            current = vec![p1];
        }

        prev_inside = inside1;
    }

    if current.len() >= 2 {
        result.push(current);
    }
    result
}

/// Fetch streets data from Overpass API for a bounding box around a point.
async fn fetch_streets_from_overpass(
    client: &reqwest::Client,
    city: &CityConfig,
    geocoder: &mut Geocoder<'_>,
    boundary: Option<&Boundary>,
) -> Result<Vec<StreetData>> {
    let CityConfig { lat, lng, bbox, .. } = *city;
    // Prefer the actual commune boundary's bounding box (padded) over the
    // configured `bbox` square: communes are rarely square, and a fixed
    // radius around the center point can clip off real streets near an
    // elongated edge (the boundary itself still gets applied afterwards via
    // `clip_raw_way`, so padding generously here is harmless).
    const BOUNDARY_MARGIN_DEG: f64 = 0.003;
    let (min_lat, min_lng, max_lat, max_lng) = match boundary {
        Some(boundary) => boundary_bbox(boundary, BOUNDARY_MARGIN_DEG),
        None => (lat - bbox, lng - bbox, lat + bbox, lng + bbox),
    };
    let query = format!(
        "[out:json][timeout:60];\n\
        (\n\
          way[\"highway\"~\"^(residential|secondary|tertiary|primary|living_street|pedestrian|unclassified)$\"]\n\
            ({min_lat},{min_lng},{max_lat},{max_lng});\n\
        );\n\
        out body geom;"
    );

    let data = run_overpass_query(client, &query).await?;

    // First pass: parse every way, keeping its OSM `name` tag (if any) as
    // `None` for the unnamed ones we'll need to resolve below.
    let mut raw: Vec<RawWay> = Vec::new();
    for element in data.elements {
        if element.kind != "way" {
            continue;
        }
        let Some(geometry) = element.geometry else {
            continue;
        };
        let nodes: Vec<(f64, f64)> = geometry.iter().map(|node| (node.lat, node.lon)).collect();
        if nodes.len() < 2 {
            continue;
        }
        let length = calculate_distance(&nodes);
        let name = element
            .tags
            .and_then(|tags| tags.name)
            .filter(|name| !name.trim().is_empty());
        // Keep node ids only when they align 1:1 with the geometry.
        let node_ids = element
            .nodes
            .filter(|ids| ids.len() == nodes.len())
            .unwrap_or_default();
        raw.push(RawWay {
            id: element.id,
            nodes,
            length,
            node_ids,
            name,
            piece: None,
        });
    }

    // Clip every way to the commune's administrative boundary, dropping the
    // portions (and whole ways) that fall outside it. A way that crosses the
    // border is cut into one or more in-boundary pieces, each ending exactly
    // at the border; the route solver already turns around at dead ends, so
    // this alone makes the route stop at the border instead of wandering
    // into the neighboring commune.
    if let Some(boundary) = boundary {
        raw = raw
            .into_iter()
            .flat_map(|way| clip_raw_way(way, boundary))
            .collect();
    }

    // Second pass: resolve a name for the unnamed ways. Most unnamed ways
    // are short spurs/fragments of an already-named street (e.g. a
    // driveway cut or a split way at an intersection), so first try to
    // snap to the nearest *already-named* way within `SNAP_DISTANCE_M`
    // (pure local geometry, no network call). Only ways with nothing
    // nearby fall back to a throttled, disk-cached Nominatim lookup.
    let named_indices: Vec<usize> = (0..raw.len()).filter(|&i| raw[i].name.is_some()).collect();
    let mut names: Vec<Option<String>> = raw.iter().map(|way| way.name.clone()).collect();

    for i in 0..raw.len() {
        if names[i].is_some() {
            continue;
        }
        let (mid_lat, mid_lng) = raw[i].nodes[raw[i].nodes.len() / 2];
        let snapped = named_indices
            .iter()
            .filter_map(|&j| {
                let distance = point_to_polyline_distance_m((mid_lat, mid_lng), &raw[j].nodes, lat);
                (distance <= SNAP_DISTANCE_M).then(|| (distance, raw[j].name.as_deref().unwrap()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, name)| name.to_string());

        names[i] = Some(match snapped {
            Some(name) => name,
            // Truly isolated unnamed way (no nearby named street): ask
            // Nominatim for the road name at this way's midpoint instead of
            // exposing the raw OSM id.
            None => geocoder
                .road_name(mid_lat, mid_lng)
                .await
                .unwrap_or_else(|| format!("Street {}", raw[i].id)),
        });
    }

    let streets = raw
        .into_iter()
        .zip(names)
        .map(|(way, name)| StreetData {
            id: match way.piece {
                Some(piece) => format!("way-{}-{piece}", way.id),
                None => format!("way-{}", way.id),
            },
            name: name.expect("every way was assigned a name above"),
            nodes: way.nodes,
            length: way.length,
            node_ids: way.node_ids,
        })
        .collect();

    Ok(streets)
}

/// A parsed OSM way, before name resolution (see [`fetch_streets_from_overpass`]).
struct RawWay {
    id: i64,
    nodes: Vec<(f64, f64)>,
    length: f64,
    node_ids: Vec<i64>,
    name: Option<String>,
    /// Set when this way was split by [`clip_raw_way`] into multiple
    /// in-boundary pieces, to keep their generated ids unique. `node_ids` is
    /// dropped for split pieces since the cut point isn't an OSM node, so
    /// alignment with `nodes` can't be preserved.
    piece: Option<usize>,
}

/// Clip a single raw way to `boundary`, returning the in-boundary piece(s).
/// A way entirely inside the boundary passes through unchanged (including
/// its `node_ids`, so intersection-splitting in [`split_into_segments`]
/// keeps working normally for the common case).
fn clip_raw_way(way: RawWay, boundary: &Boundary) -> Vec<RawWay> {
    let pieces = clip_polyline(&way.nodes, boundary);

    if pieces.len() == 1 && pieces[0].len() == way.nodes.len() {
        return vec![way];
    }

    pieces
        .into_iter()
        .enumerate()
        .map(|(i, nodes)| RawWay {
            id: way.id,
            length: calculate_distance(&nodes),
            nodes,
            node_ids: Vec::new(),
            name: way.name.clone(),
            piece: Some(i),
        })
        .collect()
}

/// Snap distance (meters) used to reuse a nearby named street's name for an
/// unnamed way fragment, instead of paying for a reverse-geocode call.
const SNAP_DISTANCE_M: f64 = 20.0;

/// Approximate equirectangular projection from lat/lng degrees to meters,
/// accurate enough for distances within a single city's bounding box.
fn project_local_m(lat: f64, lng: f64, ref_lat: f64) -> (f64, f64) {
    const METERS_PER_DEGREE_LAT: f64 = 111_320.0;
    let x = lng * ref_lat.to_radians().cos() * METERS_PER_DEGREE_LAT;
    let y = lat * METERS_PER_DEGREE_LAT;
    (x, y)
}

fn point_to_segment_distance_m(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

/// Shortest distance in meters from `point` (lat, lng) to any segment of
/// `polyline` (also lat, lng), via local equirectangular projection.
fn point_to_polyline_distance_m(point: (f64, f64), polyline: &[(f64, f64)], ref_lat: f64) -> f64 {
    let p = project_local_m(point.0, point.1, ref_lat);
    polyline
        .windows(2)
        .map(|w| {
            let a = project_local_m(w[0].0, w[0].1, ref_lat);
            let b = project_local_m(w[1].0, w[1].1, ref_lat);
            point_to_segment_distance_m(p, a, b)
        })
        .fold(f64::INFINITY, f64::min)
}

/// Generate a deterministic synthetic street grid for a city.
///
/// Used as a fallback when the Overpass API is unreachable (e.g. offline or
/// sandboxed environments without access to overpass-api.de). The grid
/// approximates a small city center with a regular street layout so the rest
/// of the pipeline (path optimization, stats, leaderboards) can run
/// end-to-end without network access.
fn generate_synthetic_streets(city: &CityConfig) -> Vec<StreetData> {
    let mut rng = create_rng(&format!("synthetic-{}", city.id));
    const GRID_SIZE: i32 = 5;
    let mut streets = Vec::new();

    let lat_step = (city.bbox * 2.0) / f64::from(GRID_SIZE);
    let lng_step = (city.bbox * 2.0) / f64::from(GRID_SIZE);
    let lat_start = city.lat - city.bbox;
    let lng_start = city.lng - city.bbox;

    // Horizontal streets ("avenues")
    for row in 0..=GRID_SIZE {
        let lat = lat_start + f64::from(row) * lat_step;
        let nodes = vec![(lat, lng_start), (lat, lng_start + city.bbox * 2.0)];
        streets.push(StreetData {
            id: format!("synthetic-avenue-{row}"),
            name: format!("Avenue {}", row + 1),
            length: calculate_distance(&nodes),
            nodes,
            node_ids: Vec::new(),
        });
    }

    // Vertical streets ("rues")
    for col in 0..=GRID_SIZE {
        let lng = lng_start + f64::from(col) * lng_step;
        let nodes = vec![(lat_start, lng), (lat_start + city.bbox * 2.0, lng)];
        streets.push(StreetData {
            id: format!("synthetic-rue-{col}"),
            name: format!("Rue {}", col + 1),
            length: calculate_distance(&nodes),
            nodes,
            node_ids: Vec::new(),
        });
    }

    // A handful of diagonal "impasses" / dead ends for variety.
    for i in 0..4 {
        let base_lat = lat_start + rng() * city.bbox * 2.0;
        let base_lng = lng_start + rng() * city.bbox * 2.0;
        let end_lat = base_lat + (rng() - 0.5) * lat_step;
        let end_lng = base_lng + (rng() - 0.5) * lng_step;
        let nodes = vec![(base_lat, base_lng), (end_lat, end_lng)];
        streets.push(StreetData {
            id: format!("synthetic-impasse-{i}"),
            name: format!("Impasse des Lilas {}", i + 1),
            length: calculate_distance(&nodes),
            nodes,
            node_ids: Vec::new(),
        });
    }

    streets
}

/// Forward bearing in degrees [0, 360) from `from` to `to`.
fn bearing(from: (f64, f64), to: (f64, f64)) -> f64 {
    let lat1 = from.0.to_radians();
    let lat2 = to.0.to_radians();
    let dlng = (to.1 - from.1).to_radians();
    let y = dlng.sin() * lat2.cos();
    let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * dlng.cos();
    let b = y.atan2(x).to_degrees();
    ((b % 360.0) + 360.0) % 360.0
}

/// Signed turn angle in (-180, 180]: positive = right, negative = left.
fn turn_angle(in_bearing: f64, out_bearing: f64) -> f64 {
    ((out_bearing - in_bearing + 180.0).rem_euclid(360.0)) - 180.0
}

/// Map a signed turn angle to an instruction key understood by the frontend.
fn classify_turn(angle: f64) -> &'static str {
    let abs = angle.abs();
    if abs < 20.0 {
        "straight"
    } else if abs < 60.0 {
        if angle > 0.0 {
            "slight_right"
        } else {
            "slight_left"
        }
    } else if abs < 120.0 {
        if angle > 0.0 {
            "turn_right"
        } else {
            "turn_left"
        }
    } else if abs < 160.0 {
        if angle > 0.0 {
            "sharp_right"
        } else {
            "sharp_left"
        }
    } else {
        "uturn"
    }
}

fn segment_exit_bearing(nodes: &[(f64, f64)], reversed: bool) -> Option<f64> {
    if nodes.len() < 2 {
        return None;
    }
    if reversed {
        Some(bearing(nodes[1], nodes[0]))
    } else {
        let n = nodes.len();
        Some(bearing(nodes[n - 2], nodes[n - 1]))
    }
}

fn segment_entry_bearing(nodes: &[(f64, f64)], reversed: bool) -> Option<f64> {
    if nodes.len() < 2 {
        return None;
    }
    if reversed {
        let n = nodes.len();
        Some(bearing(nodes[n - 1], nodes[n - 2]))
    } else {
        Some(bearing(nodes[0], nodes[1]))
    }
}

fn segment_start_coord(nodes: &[(f64, f64)], reversed: bool) -> (f64, f64) {
    if reversed {
        *nodes.last().unwrap()
    } else {
        nodes[0]
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RouteStepData {
    /// One of: "start", "straight", "slight_left", "slight_right",
    /// "turn_left", "turn_right", "sharp_left", "sharp_right", "uturn", "arrive".
    instruction: String,
    street_name: String,
    /// Distance in metres for this step (rounded to integer).
    distance_m: f64,
    /// Lat/lng of the maneuver point (for map focus).
    coordinate: (f64, f64),
    /// Full polyline geometry of this step (all node coordinates in order of travel).
    geometry: Vec<(f64, f64)>,
    /// Reverse-geocoded address, only set for the "start"/"arrive" steps.
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
}

fn seg_coords(nodes: &[(f64, f64)], reversed: bool) -> Vec<(f64, f64)> {
    if reversed {
        nodes.iter().rev().copied().collect()
    } else {
        nodes.to_vec()
    }
}

/// Build turn-by-turn navigation steps from the solver's segment visit sequence.
/// Consecutive visits on the same named street are merged; a turn instruction
/// is computed at each street boundary.
fn build_route_steps(
    visits: &[(usize, bool)],
    segments: &[route::StreetSegment],
    segment_to_street: &[usize],
    streets: &[StreetData],
) -> Vec<RouteStepData> {
    if visits.is_empty() {
        return Vec::new();
    }

    let mut steps: Vec<RouteStepData> = Vec::new();
    let mut group_street_idx = segment_to_street[visits[0].0];
    let mut group_distance: f64 = 0.0;
    let mut group_coord = segment_start_coord(&segments[visits[0].0].nodes, visits[0].1);
    let mut group_geometry: Vec<(f64, f64)> = Vec::new();
    let mut instruction = "start".to_string();
    let mut prev_exit: Option<f64> = None;

    for &(seg_idx, reversed) in visits {
        let seg = &segments[seg_idx];
        let street_idx = segment_to_street[seg_idx];

        if street_idx != group_street_idx {
            steps.push(RouteStepData {
                instruction: instruction.clone(),
                street_name: streets[group_street_idx].name.clone(),
                distance_m: group_distance.round(),
                coordinate: group_coord,
                geometry: std::mem::take(&mut group_geometry),
                address: None,
            });
            let entry = segment_entry_bearing(&seg.nodes, reversed);
            instruction = match (prev_exit, entry) {
                (Some(exit), Some(ent)) => classify_turn(turn_angle(exit, ent)).to_string(),
                _ => "straight".to_string(),
            };
            group_street_idx = street_idx;
            group_distance = 0.0;
            group_coord = segment_start_coord(&seg.nodes, reversed);
        }

        // Append this segment's nodes, skipping the first when chaining (it
        // duplicates the previous segment's last point).
        let coords = seg_coords(&seg.nodes, reversed);
        if group_geometry.is_empty() {
            group_geometry = coords;
        } else {
            group_geometry.extend_from_slice(&coords[1..]);
        }

        group_distance += seg.length * 1000.0;
        prev_exit = segment_exit_bearing(&seg.nodes, reversed);
    }

    steps.push(RouteStepData {
        instruction,
        street_name: streets[group_street_idx].name.clone(),
        distance_m: group_distance.round(),
        coordinate: group_coord,
        geometry: group_geometry,
        address: None,
    });

    // Final "arrive" marker at the last coordinate visited.
    let &(last_idx, last_rev) = visits.last().unwrap();
    let last_nodes = &segments[last_idx].nodes;
    let arrive = if last_rev {
        last_nodes[0]
    } else {
        *last_nodes.last().unwrap()
    };
    steps.push(RouteStepData {
        instruction: "arrive".to_string(),
        street_name: String::new(),
        distance_m: 0.0,
        coordinate: arrive,
        geometry: Vec::new(),
        address: None,
    });

    steps
}

/// Compute the optimised route and build turn-by-turn navigation steps.
/// Reverse-geocode the route's first ("start") and last ("arrive") steps so
/// the panel can show the exact address instead of just "Départ"/"Arrivée".
async fn fill_endpoint_addresses(geocoder: &mut Geocoder<'_>, steps: &mut [RouteStepData]) {
    if let Some(first) = steps.first_mut() {
        let (lat, lng) = first.coordinate;
        first.address = geocoder.reverse_address(lat, lng).await;
    }
    if let Some(last) = steps.last_mut() {
        let (lat, lng) = last.coordinate;
        last.address = geocoder.reverse_address(lat, lng).await;
    }
}

fn optimize_and_plan_route(streets: &[StreetData]) -> (route::RouteResult, Vec<RouteStepData>) {
    let (segments, segment_to_street) = split_into_segments(streets);
    let result = route::optimize_route(&segments);
    let steps = build_route_steps(
        &result.segment_visits,
        &segments,
        &segment_to_street,
        streets,
    );
    (result, steps)
}

/// Split each OSM way into edges at every node it shares with another way, so
/// that mid-way intersections become real graph vertices. Without this, two
/// streets that cross only at an interior node look disconnected (the graph
/// only joins ways at their endpoints), fragmenting the network into hundreds
/// of components and inflating the route with out-and-back detours.
///
/// Streets without node ids (synthetic data) pass through as a single segment
/// identified by their endpoint coordinates.
fn split_into_segments(streets: &[StreetData]) -> (Vec<route::StreetSegment>, Vec<usize>) {
    // How many ways each node id belongs to; a node shared by >= 2 ways is an
    // intersection we must split at.
    let mut usage: HashMap<i64, u32> = HashMap::new();
    for street in streets {
        for &id in &street.node_ids {
            *usage.entry(id).or_default() += 1;
        }
    }

    let mut segments: Vec<route::StreetSegment> = Vec::new();
    let mut segment_to_street: Vec<usize> = Vec::new();

    for (street_idx, street) in streets.iter().enumerate() {
        if street.node_ids.len() != street.nodes.len() || street.node_ids.len() < 2 {
            segments.push(route::StreetSegment {
                nodes: street.nodes.clone(),
                length: street.length,
                from_id: None,
                to_id: None,
            });
            segment_to_street.push(street_idx);
            continue;
        }

        // Split points: the two endpoints plus any interior node shared with
        // another way.
        let last = street.node_ids.len() - 1;
        let split_at: Vec<usize> = (0..street.node_ids.len())
            .filter(|&i| i == 0 || i == last || usage[&street.node_ids[i]] >= 2)
            .collect();

        for pair in split_at.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            let nodes = street.nodes[start..=end].to_vec();
            segments.push(route::StreetSegment {
                length: calculate_distance(&nodes),
                from_id: Some(street.node_ids[start]),
                to_id: Some(street.node_ids[end]),
                nodes,
            });
            segment_to_street.push(street_idx);
        }
    }

    (segments, segment_to_street)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayerResultData {
    player_id: String,
    player_name: String,
    country: String,
    city_id: String,
    distance: f64,
    completed: bool,
    rank: i64,
    time: i64,
}

/// Generate synthetic, deterministic player results for a given edition.
fn generate_player_results(
    edition_id: &str,
    city_id: &str,
    total_distance_km: f64,
) -> Vec<PlayerResultData> {
    let mut results = Vec::new();

    for player in PLAYERS {
        let mut rng = create_rng(&format!("{edition_id}-{}", player.id));

        // Not every player attempts every city.
        if rng() > 0.85 {
            continue;
        }

        let completion_factor = 0.6 + rng() * 0.5; // 60% to 110% of the route
        let distance = (total_distance_km * completion_factor.min(1.08) * 100.0).round() / 100.0;
        let completed = is_all_streets_completed(distance, total_distance_km);
        let pace_seconds_per_km = 280.0 + rng() * 200.0; // between ~4'40 and ~8'00 per km
        let time = (distance * pace_seconds_per_km).round() as i64;

        results.push(PlayerResultData {
            player_id: player.id.to_string(),
            player_name: player.name.to_string(),
            country: player.country.to_string(),
            city_id: city_id.to_string(),
            distance,
            completed,
            rank: 0,
            time,
        });
    }

    // Rank by distance covered (desc), tie-break by fastest time.
    results.sort_by(|a, b| {
        b.distance
            .partial_cmp(&a.distance)
            .unwrap()
            .then(a.time.cmp(&b.time))
    });
    for (index, result) in results.iter_mut().enumerate() {
        result.rank = index as i64 + 1;
    }

    results
}

fn format_distance(km: f64) -> f64 {
    (km * 1000.0).round() / 1000.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditionSummary {
    id: String,
    current: bool,
    country: String,
    region: String,
    department: String,
    postal_code: String,
    name: String,
    date: String,
    street_count: i64,
    total_meters: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PathFile {
    id: String,
    city_id: String,
    /// Optimised route visiting every street (may repeat streets at dead ends
    /// or between disconnected components). Used for distance calculation and
    /// start/end pin placement; NOT used for map rendering.
    coordinates: Vec<(f64, f64)>,
    /// Each unique street's geometry, in the order streets were fetched from
    /// Overpass. Used for map rendering: each street is drawn exactly once,
    /// regardless of how many times the route traverses it.
    street_geometries: Vec<Vec<(f64, f64)>>,
    /// Turn-by-turn navigation steps for the route panel.
    route_steps: Vec<RouteStepData>,
    total_distance: f64,
    street_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatsFile {
    city_id: String,
    edition_id: String,
    date: String,
    street_count: i64,
    total_meters: i64,
    total_distance: f64,
    total_attempts: i64,
    total_completed: i64,
    leaderboard: Vec<PlayerResultData>,
}

struct Paths {
    cities_registry: PathBuf,
    data_raw: PathBuf,
    public_data: PathBuf,
}

impl Paths {
    fn new() -> Self {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let frontend_dir = manifest_dir.join("../frontend");
        Paths {
            cities_registry: manifest_dir.join("cities.json"),
            data_raw: frontend_dir.join("data-raw"),
            public_data: frontend_dir.join("public/data"),
        }
    }
}

fn city_dir(city: &CityConfig) -> PathBuf {
    PathBuf::from(&city.country[..2])
        .join(&city.department)
        .join(&city.postal_code)
        .join(&city.name)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, json)?;
    Ok(())
}

/// Process a single city: fetch streets, write path/stats files, and return
/// a summary used for the city index and the global manifest.
async fn process_city(
    city: &CityConfig,
    streets: Vec<StreetData>,
    paths: &Paths,
    geocoder: &mut Geocoder<'_>,
) -> Result<(EditionSummary, String)> {
    let relative_dir = city_dir(city);
    let absolute_dir = paths.public_data.join(&relative_dir);
    fs::create_dir_all(absolute_dir.join("paths"))?;
    fs::create_dir_all(absolute_dir.join("stats"))?;

    let date = Utc::now().format("%Y-%m-%d").to_string();
    let edition_id = format!("{}-{date}", city.id);

    let total_meters = streets
        .iter()
        .map(|s| s.length * 1000.0)
        .sum::<f64>()
        .round() as i64;
    let total_distance_km = format_distance(total_meters as f64 / 1000.0);

    // Save raw OSM data for reproducibility.
    fs::create_dir_all(&paths.data_raw)?;
    write_json(&paths.data_raw.join(format!("{edition_id}.json")), &streets)?;

    // Diff against the previous edition, if any.
    let index_path = absolute_dir.join("index.json");
    let previous_editions: Vec<EditionSummary> = if index_path.exists() {
        serde_json::from_str(&fs::read_to_string(&index_path)?)?
    } else {
        Vec::new()
    };
    let previous = previous_editions.last();

    let street_diff = streets.len() as i64 - previous.map_or(0, |p| p.street_count);
    let meters_diff = total_meters - previous.map_or(0, |p| p.total_meters);
    println!(
        "  {} streets ({}{street_diff}), {total_meters} m ({}{meters_diff} m)",
        streets.len(),
        if street_diff >= 0 { "+" } else { "" },
        if meters_diff >= 0 { "+" } else { "" },
    );

    // Path data: the optimized route covering every street (may exceed the
    // network's total length when dead ends or disconnected streets force
    // backtracking/transfers).
    let (route, mut route_steps) = optimize_and_plan_route(&streets);
    fill_endpoint_addresses(geocoder, &mut route_steps).await;
    write_json(
        &absolute_dir
            .join("paths")
            .join(format!("{edition_id}.json")),
        &PathFile {
            id: edition_id.clone(),
            city_id: city.id.to_string(),
            coordinates: route.coordinates,
            street_geometries: streets.iter().map(|s| s.nodes.clone()).collect(),
            route_steps,
            total_distance: format_distance(route.total_distance_km),
            street_count: streets.len() as i64,
        },
    )?;

    // Stats data: leaderboard + aggregated metrics for this edition.
    let leaderboard = generate_player_results(&edition_id, &city.id, total_distance_km);
    let total_completed = leaderboard.iter().filter(|r| r.completed).count() as i64;
    write_json(
        &absolute_dir
            .join("stats")
            .join(format!("{edition_id}.json")),
        &StatsFile {
            city_id: city.id.to_string(),
            edition_id: edition_id.clone(),
            date: date.clone(),
            street_count: streets.len() as i64,
            total_meters,
            total_distance: total_distance_km,
            total_attempts: leaderboard.len() as i64,
            total_completed,
            leaderboard,
        },
    )?;

    // Index: idempotent — replace today's edition if it already exists.
    let edition = EditionSummary {
        id: edition_id.clone(),
        current: true,
        country: city.country.to_string(),
        region: city.region.to_string(),
        department: city.department.to_string(),
        postal_code: city.postal_code.to_string(),
        name: city.name.to_string(),
        date,
        street_count: streets.len() as i64,
        total_meters,
    };
    let mut editions: Vec<EditionSummary> = previous_editions
        .into_iter()
        .filter(|e| e.id != edition_id)
        .map(|mut e| {
            e.current = false;
            e
        })
        .collect();
    editions.push(edition.clone());
    write_json(&index_path, &editions)?;

    println!("  -> wrote {}", absolute_dir.display());

    Ok((edition, relative_dir.to_string_lossy().replace('\\', "/")))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CityManifestEntry {
    id: String,
    country: String,
    region: String,
    department: String,
    postal_code: String,
    name: String,
    date: String,
    street_count: i64,
    total_meters: i64,
    dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Achievement {
    #[serde(rename = "type")]
    kind: AchievementType,
    city_id: String,
    date: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlayerManifestEntry {
    id: String,
    name: String,
    country: String,
    total_distance: f64,
    cities_completed: i64,
    achievements: Vec<Achievement>,
    results: Vec<PlayerResultData>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LeaderboardEntry {
    rank: usize,
    player_id: String,
    player_name: String,
    country: String,
    cities_completed: i64,
    total_distance: f64,
}

struct PlayerAccumulator {
    id: String,
    name: String,
    country: String,
    total_distance: f64,
    cities_completed: i64,
    completed_distance: f64,
    achievements: Vec<Achievement>,
    results: Vec<PlayerResultData>,
}

struct RankingDraft {
    player_id: String,
    player_name: String,
    country: String,
    cities_completed: i64,
    total_distance: f64,
    total_points: i64,
}

impl HasPoints for RankingDraft {
    fn total_points(&self) -> i64 {
        self.total_points
    }
}

/// Aggregate per-city stats into global manifests consumed by the Players
/// and Leaderboard pages.
fn build_global_manifests(
    processed: &[(&CityConfig, EditionSummary, String)],
    paths: &Paths,
) -> Result<()> {
    // Top-level city manifest, used by the Cities page.
    let cities: Vec<CityManifestEntry> = processed
        .iter()
        .map(|(city, edition, dir)| CityManifestEntry {
            id: city.id.to_string(),
            country: edition.country.clone(),
            region: edition.region.clone(),
            department: edition.department.clone(),
            postal_code: edition.postal_code.clone(),
            name: edition.name.clone(),
            date: edition.date.clone(),
            street_count: edition.street_count,
            total_meters: edition.total_meters,
            dir: dir.clone(),
        })
        .collect();
    write_json(&paths.public_data.join("cities.json"), &cities)?;

    // Aggregate every player's results across all city editions processed.
    let mut player_stats: HashMap<&'static str, PlayerAccumulator> = HashMap::new();

    for (city, edition, _dir) in processed {
        let stats_path = paths
            .public_data
            .join(city_dir(city))
            .join("stats")
            .join(format!("{}.json", edition.id));
        let stats: StatsFile = serde_json::from_str(&fs::read_to_string(&stats_path)?)?;

        let fastest = stats
            .leaderboard
            .iter()
            .filter(|r| r.completed)
            .min_by_key(|r| r.time);

        for result in &stats.leaderboard {
            let Some(player) = PLAYERS.iter().find(|p| p.id == result.player_id) else {
                continue;
            };

            let entry = player_stats
                .entry(player.id)
                .or_insert_with(|| PlayerAccumulator {
                    id: player.id.to_string(),
                    name: player.name.to_string(),
                    country: player.country.to_string(),
                    total_distance: 0.0,
                    cities_completed: 0,
                    completed_distance: 0.0,
                    achievements: Vec::new(),
                    results: Vec::new(),
                });

            entry.total_distance += result.distance;
            entry.results.push(result.clone());

            if result.completed {
                entry.cities_completed += 1;
                entry.completed_distance += stats.total_distance;
                entry.achievements.push(Achievement {
                    kind: AchievementType::AllStreets,
                    city_id: city.id.to_string(),
                    date: edition.date.clone(),
                });
            }
            if result.rank == 1 {
                entry.achievements.push(Achievement {
                    kind: AchievementType::FirstCity,
                    city_id: city.id.to_string(),
                    date: edition.date.clone(),
                });
            }
            if let Some(fastest) = fastest {
                if result.player_id == fastest.player_id {
                    entry.achievements.push(Achievement {
                        kind: AchievementType::Fastest,
                        city_id: city.id.to_string(),
                        date: edition.date.clone(),
                    });
                }
            }
        }
    }

    let players: Vec<PlayerManifestEntry> = player_stats
        .values()
        .map(|p| PlayerManifestEntry {
            id: p.id.clone(),
            name: p.name.clone(),
            country: p.country.clone(),
            total_distance: format_distance(p.total_distance),
            cities_completed: p.cities_completed,
            achievements: p.achievements.clone(),
            results: p.results.clone(),
        })
        .collect();
    write_json(&paths.public_data.join("players.json"), &players)?;

    // Global points leaderboard.
    let drafts: Vec<RankingDraft> = player_stats
        .values()
        .map(|p| RankingDraft {
            player_id: p.id.clone(),
            player_name: p.name.clone(),
            country: p.country.clone(),
            cities_completed: p.cities_completed,
            total_distance: format_distance(p.completed_distance),
            total_points: calculate_total_points(p.cities_completed, p.completed_distance)
                + calculate_achievement_bonus_points(
                    &p.achievements.iter().map(|a| a.kind).collect::<Vec<_>>(),
                ),
        })
        .collect();

    let ranked: Vec<LeaderboardEntry> = rank_entries(drafts)
        .into_iter()
        .map(|(entry, rank)| LeaderboardEntry {
            rank,
            player_id: entry.player_id,
            player_name: entry.player_name,
            country: entry.country,
            cities_completed: entry.cities_completed,
            total_distance: entry.total_distance,
        })
        .collect();
    write_json(&paths.public_data.join("leaderboard.json"), &ranked)?;

    Ok(())
}

/// Remove a town's generated manifest directory and cached raw Overpass
/// fetches. The town must still have an entry in `cities.json` (its
/// country/department/postal_code/name are needed to resolve the directory)
/// — remove the entry afterwards and rebuild with no id to refresh the
/// global manifests.
fn delete_city(city_id: &str) -> Result<()> {
    let paths = Paths::new();
    let city_registry = load_city_registry(&paths.cities_registry)?;

    let Some(city) = city_registry.iter().find(|c| c.id == city_id) else {
        eprintln!(
            "Error: {city_id} is not in {}",
            paths.cities_registry.display()
        );
        eprintln!(
            "Available ids: {}",
            city_registry
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(1);
    };

    let dir = paths.public_data.join(city_dir(city));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
        println!("Removed {}", dir.display());
    } else {
        println!("{} does not exist, nothing to remove", dir.display());
    }

    if paths.data_raw.exists() {
        let prefix = format!("{city_id}-");
        for entry in fs::read_dir(&paths.data_raw)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                fs::remove_file(entry.path())?;
                println!("Removed {}", entry.path().display());
            }
        }
    }

    println!(
        "\nDone. Now remove the `{city_id}` entry from {}, then run \
         `just backend::city-build` to refresh the global manifests.",
        paths.cities_registry.display()
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if let Some(pos) = args.iter().position(|a| a == "--delete") {
        let Some(city_id) = args.get(pos + 1) else {
            eprintln!("Usage: build-data --delete CITY_ID");
            std::process::exit(1);
        };
        return delete_city(city_id);
    }

    // `--allow-synthetic` opts in to the deterministic fake street grid when
    // Overpass is unreachable (e.g. offline CI). By default a fetch failure is
    // a hard error rather than silently producing grid data.
    let allow_synthetic = args.iter().any(|a| a == "--allow-synthetic");
    let requested_ids: Vec<String> = args.into_iter().filter(|a| !a.starts_with("--")).collect();

    let paths = Paths::new();
    let city_registry = load_city_registry(&paths.cities_registry)?;

    let cities: Vec<&CityConfig> = if requested_ids.is_empty() {
        city_registry.iter().collect()
    } else {
        city_registry
            .iter()
            .filter(|c| requested_ids.iter().any(|id| id.as_str() == c.id))
            .collect()
    };

    if cities.is_empty() {
        eprintln!("No matching cities for: {}", requested_ids.join(", "));
        eprintln!(
            "Available ids: {}",
            city_registry
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(1);
    }

    println!("Starting data build for {} city/cities...", cities.len());

    // Overpass mirrors reject requests without a User-Agent with HTTP 406, so
    // an explicit UA is mandatory — without it every fetch fails and the build
    // silently falls back to the synthetic grid.
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "city-challenge/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/trois-six/city-challenge)"
        ))
        .build()?;
    let mut processed: Vec<(&CityConfig, EditionSummary, String)> = Vec::new();
    let mut geocoder = Geocoder::new(&client, paths.data_raw.join("geocode-cache.json"));

    for city in &cities {
        println!("\nProcessing {} ({})...", city.name, city.postal_code);

        let boundary = match fetch_city_boundary(&client, city).await {
            Ok(Some(boundary)) => {
                println!(
                    "  fetched commune boundary ({} ring(s))",
                    boundary.rings.len()
                );
                Some(boundary)
            }
            Ok(None) => {
                println!(
                    "  Warning: no commune boundary found for {}; streets won't be clipped at the city border",
                    city.name
                );
                None
            }
            Err(error) => {
                println!(
                    "  Warning: failed to fetch commune boundary for {} ({error}); streets won't be clipped at the city border",
                    city.name
                );
                None
            }
        };

        // Overpass rate-limits bursts of requests from the same client; a
        // short pause between the boundary query above and the street query
        // below avoids tripping it.
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

        let streets = match fetch_streets_from_overpass(
            &client,
            city,
            &mut geocoder,
            boundary.as_ref(),
        )
        .await
        {
            Ok(streets) if !streets.is_empty() => {
                println!("  fetched {} streets from Overpass", streets.len());
                streets
            }
            Ok(_) if allow_synthetic => {
                println!("  Overpass API returned no streets, using synthetic street grid");
                generate_synthetic_streets(city)
            }
            Err(error) if allow_synthetic => {
                println!("  Overpass API unavailable ({error}), using synthetic street grid");
                generate_synthetic_streets(city)
            }
            Ok(_) => {
                eprintln!("Error: Overpass returned no streets for {} and --allow-synthetic was not set; skipping", city.name);
                continue;
            }
            Err(error) => {
                eprintln!("Error: Overpass fetch failed for {} ({error}) and --allow-synthetic was not set; skipping", city.name);
                continue;
            }
        };

        match process_city(city, streets, &paths, &mut geocoder).await {
            Ok((edition, dir)) => processed.push((city, edition, dir)),
            Err(error) => eprintln!("Error processing {}: {error}", city.name),
        }

        // Persist after every city so an interrupted build doesn't lose the
        // (rate-limited) lookups it already paid for.
        geocoder.save()?;
    }

    // Re-build global manifests from the full registry so unaffected cities
    // keep their data even when building a subset.
    let mut all_processed: Vec<(&CityConfig, EditionSummary, String)> = Vec::new();
    for city in &city_registry {
        let dir = city_dir(city);
        let index_path = paths.public_data.join(&dir).join("index.json");
        if !index_path.exists() {
            continue;
        }
        let editions: Vec<EditionSummary> =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        let current = editions
            .iter()
            .find(|e| e.current)
            .or_else(|| editions.last())
            .cloned();
        if let Some(current) = current {
            all_processed.push((city, current, dir.to_string_lossy().replace('\\', "/")));
        }
    }

    if !all_processed.is_empty() {
        build_global_manifests(&all_processed, &paths)?;
    }

    println!(
        "\nData build complete! Processed {}/{} requested cities, {} cities in global manifests.",
        processed.len(),
        cities.len(),
        all_processed.len()
    );

    Ok(())
}
