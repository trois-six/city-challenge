use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct City {
    pub id: String,
    pub country: String,
    pub region: String,
    pub department: String,
    pub postal_code: String,
    pub name: String,
    pub date: String,
    pub street_count: i32,
    pub total_meters: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub country: String,
    pub total_distance: f64,
    pub cities_completed: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerResult {
    pub player_id: String,
    pub player_name: String,
    pub country: String,
    pub city_id: String,
    pub distance: f64,
    pub completed: bool,
    pub rank: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathData {
    pub id: String,
    pub city_id: String,
    pub coordinates: Vec<[f64; 2]>,
    pub total_distance: f64,
    pub street_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: i32,
    pub player_id: String,
    pub player_name: String,
    pub country: String,
    pub cities_completed: i32,
    pub total_distance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub city_id: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CommentRequest {
    pub text: String,
}
