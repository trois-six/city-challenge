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

/// Rate-limited Nominatim reverse-geocoding client. Nominatim's usage policy
/// caps anonymous traffic at 1 request/second, so every call goes through
/// `throttle` first; calls are sequential by construction (this is `&mut
/// self`), so a single last-call timestamp is enough.
struct Geocoder<'a> {
    client: &'a reqwest::Client,
    last_call: Option<std::time::Instant>,
}

impl<'a> Geocoder<'a> {
    fn new(client: &'a reqwest::Client) -> Self {
        Self {
            client,
            last_call: None,
        }
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
    async fn reverse(&mut self, lat: f64, lng: f64) -> Option<NominatimResponse> {
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

    /// Reverse-geocode a point into a formatted address string.
    async fn reverse_address(&mut self, lat: f64, lng: f64) -> Option<String> {
        let response = self.reverse(lat, lng).await?;
        response.address.format(response.display_name.as_deref())
    }
}

/// Fetch streets data from Overpass API for a bounding box around a point.
async fn fetch_streets_from_overpass(
    client: &reqwest::Client,
    city: &CityConfig,
    geocoder: &mut Geocoder<'_>,
) -> Result<Vec<StreetData>> {
    let CityConfig { lat, lng, bbox, .. } = *city;
    let query = format!(
        "[out:json][timeout:60];\n\
        (\n\
          way[\"highway\"~\"^(residential|secondary|tertiary|primary|living_street|service|footway|path|track|pedestrian|cycleway|unclassified)$\"]\n\
            ({},{},{},{});\n\
        );\n\
        out body geom;",
        lat - bbox,
        lng - bbox,
        lat + bbox,
        lng + bbox
    );

    // overpass-api.de load-balances across several backends and occasionally
    // routes a request to one returning a transient error (e.g. HTTP 406);
    // retrying a handful of times with a short backoff works around this.
    const MAX_ATTEMPTS: u32 = 4;
    let mut last_error = None;
    let mut data: Option<OverpassResponse> = None;

    for attempt in 1..=MAX_ATTEMPTS {
        let result = async {
            let response = client
                .post(OVERPASS_API_URL)
                .form(&[("data", query.as_str())])
                .send()
                .await?
                .error_for_status()?;
            response.json::<OverpassResponse>().await
        }
        .await;

        match result {
            Ok(response) => {
                data = Some(response);
                break;
            }
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

    let data = match data {
        Some(data) => data,
        None => return Err(last_error.expect("at least one attempt was made").into()),
    };

    let mut streets = Vec::new();
    for element in data.elements {
        if element.kind == "way" {
            if let Some(geometry) = element.geometry {
                let nodes: Vec<(f64, f64)> =
                    geometry.iter().map(|node| (node.lat, node.lon)).collect();
                if nodes.len() >= 2 {
                    let length = calculate_distance(&nodes);
                    let name = match element.tags.and_then(|tags| tags.name) {
                        Some(name) => name,
                        // No `name` tag (common for unnamed footways/service
                        // roads): ask Nominatim for the road name at this
                        // way's midpoint instead of exposing the raw OSM id.
                        None => {
                            let (mid_lat, mid_lng) = nodes[nodes.len() / 2];
                            geocoder
                                .reverse(mid_lat, mid_lng)
                                .await
                                .and_then(|response| {
                                    response.address.road_name().map(str::to_string)
                                })
                                .unwrap_or_else(|| format!("Street {}", element.id))
                        }
                    };
                    // Keep node ids only when they align 1:1 with the geometry.
                    let node_ids = element
                        .nodes
                        .filter(|ids| ids.len() == nodes.len())
                        .unwrap_or_default();
                    streets.push(StreetData {
                        id: format!("way-{}", element.id),
                        name,
                        nodes,
                        length,
                        node_ids,
                    });
                }
            }
        }
    }

    Ok(streets)
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
    let mut geocoder = Geocoder::new(&client);

    for city in &cities {
        println!("\nProcessing {} ({})...", city.name, city.postal_code);

        let streets = match fetch_streets_from_overpass(&client, city, &mut geocoder).await {
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
