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

use anyhow::Result;
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

/// Registry of cities to process. Pass one or more ids as CLI arguments to
/// restrict the build to a subset, e.g.:
///   cargo run --release --bin build-data -- FR-75001-Paris
struct CityConfig {
    /// `{PAYS}-{CODE_POSTAL}-{VILLE}`
    id: &'static str,
    /// ISO 3166-1 alpha-3 country code
    country: &'static str,
    region: &'static str,
    department: &'static str,
    postal_code: &'static str,
    name: &'static str,
    lat: f64,
    lng: f64,
    /// half-width of the bounding box, in degrees
    bbox: f64,
}

const CITY_REGISTRY: &[CityConfig] = &[
    CityConfig {
        id: "FR-75001-Paris",
        country: "FRA",
        region: "Île-de-France",
        department: "75",
        postal_code: "75001",
        name: "Paris",
        lat: 48.8606,
        lng: 2.3376,
        bbox: 0.006,
    },
    CityConfig {
        id: "FR-69001-Lyon",
        country: "FRA",
        region: "Auvergne-Rhône-Alpes",
        department: "69",
        postal_code: "69001",
        name: "Lyon",
        lat: 45.7679,
        lng: 4.833,
        bbox: 0.006,
    },
    CityConfig {
        id: "FR-74290-Talloires",
        country: "FRA",
        region: "Auvergne-Rhône-Alpes",
        department: "74",
        postal_code: "74290",
        name: "Talloires",
        lat: 45.8333,
        lng: 6.2167,
        bbox: 0.012,
    },
];

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

/// Fetch streets data from Overpass API for a bounding box around a point.
async fn fetch_streets_from_overpass(
    client: &reqwest::Client,
    city: &CityConfig,
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
                    let name = element
                        .tags
                        .and_then(|tags| tags.name)
                        .unwrap_or_else(|| format!("Street {}", element.id));
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

/// Convert the fetched/synthetic streets into the route solver's input and
/// compute a walk covering every street (Route Inspection / Chinese Postman
/// problem). See [`city_challenge_backend::route`] for the algorithm.
fn optimize_route(streets: &[StreetData]) -> route::RouteResult {
    let segments = split_into_segments(streets);
    route::optimize_route(&segments)
}

/// Split each OSM way into edges at every node it shares with another way, so
/// that mid-way intersections become real graph vertices. Without this, two
/// streets that cross only at an interior node look disconnected (the graph
/// only joins ways at their endpoints), fragmenting the network into hundreds
/// of components and inflating the route with out-and-back detours.
///
/// Streets without node ids (synthetic data) pass through as a single segment
/// identified by their endpoint coordinates.
fn split_into_segments(streets: &[StreetData]) -> Vec<route::StreetSegment> {
    // How many ways each node id belongs to; a node shared by >= 2 ways is an
    // intersection we must split at.
    let mut usage: HashMap<i64, u32> = HashMap::new();
    for street in streets {
        for &id in &street.node_ids {
            *usage.entry(id).or_default() += 1;
        }
    }

    let mut segments = Vec::new();
    for street in streets {
        if street.node_ids.len() != street.nodes.len() || street.node_ids.len() < 2 {
            segments.push(route::StreetSegment {
                nodes: street.nodes.clone(),
                length: street.length,
                from_id: None,
                to_id: None,
            });
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
        }
    }

    segments
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
    coordinates: Vec<(f64, f64)>,
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
    data_raw: PathBuf,
    public_data: PathBuf,
}

impl Paths {
    fn new() -> Self {
        let frontend_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frontend");
        Paths {
            data_raw: frontend_dir.join("data-raw"),
            public_data: frontend_dir.join("public/data"),
        }
    }
}

fn city_dir(city: &CityConfig) -> PathBuf {
    PathBuf::from(&city.country[..2])
        .join(city.department)
        .join(city.postal_code)
        .join(city.name)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, json)?;
    Ok(())
}

/// Process a single city: fetch streets, write path/stats files, and return
/// a summary used for the city index and the global manifest.
fn process_city(
    city: &CityConfig,
    streets: Vec<StreetData>,
    paths: &Paths,
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
    let route = optimize_route(&streets);
    write_json(
        &absolute_dir
            .join("paths")
            .join(format!("{edition_id}.json")),
        &PathFile {
            id: edition_id.clone(),
            city_id: city.id.to_string(),
            coordinates: route.coordinates,
            total_distance: format_distance(route.total_distance_km),
            street_count: streets.len() as i64,
        },
    )?;

    // Stats data: leaderboard + aggregated metrics for this edition.
    let leaderboard = generate_player_results(&edition_id, city.id, total_distance_km);
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

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // `--allow-synthetic` opts in to the deterministic fake street grid when
    // Overpass is unreachable (e.g. offline CI). By default a fetch failure is
    // a hard error rather than silently producing grid data.
    let allow_synthetic = args.iter().any(|a| a == "--allow-synthetic");
    let requested_ids: Vec<String> = args
        .into_iter()
        .filter(|a| !a.starts_with("--"))
        .collect();
    let cities: Vec<&CityConfig> = if requested_ids.is_empty() {
        CITY_REGISTRY.iter().collect()
    } else {
        CITY_REGISTRY
            .iter()
            .filter(|c| requested_ids.iter().any(|id| id == c.id))
            .collect()
    };

    if cities.is_empty() {
        eprintln!("No matching cities for: {}", requested_ids.join(", "));
        eprintln!(
            "Available ids: {}",
            CITY_REGISTRY
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(1);
    }

    println!("Starting data build for {} city/cities...", cities.len());

    let paths = Paths::new();
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

    for city in &cities {
        println!("\nProcessing {} ({})...", city.name, city.postal_code);

        let streets = match fetch_streets_from_overpass(&client, city).await {
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

        match process_city(city, streets, &paths) {
            Ok((edition, dir)) => processed.push((city, edition, dir)),
            Err(error) => eprintln!("Error processing {}: {error}", city.name),
        }
    }

    // Re-build global manifests from the full registry so unaffected cities
    // keep their data even when building a subset.
    let mut all_processed: Vec<(&CityConfig, EditionSummary, String)> = Vec::new();
    for city in CITY_REGISTRY {
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
