use crate::models::LeaderboardEntry;
use actix_web::HttpResponse;

pub async fn get_leaderboard() -> HttpResponse {
    let entries = vec![
        LeaderboardEntry {
            rank: 1,
            player_id: "player-1".to_string(),
            player_name: "Alice".to_string(),
            country: "FR".to_string(),
            cities_completed: 5,
            total_distance: 156.8,
        },
        LeaderboardEntry {
            rank: 2,
            player_id: "player-2".to_string(),
            player_name: "Bob".to_string(),
            country: "FR".to_string(),
            cities_completed: 3,
            total_distance: 128.3,
        },
        LeaderboardEntry {
            rank: 3,
            player_id: "player-3".to_string(),
            player_name: "Charlie".to_string(),
            country: "BE".to_string(),
            cities_completed: 2,
            total_distance: 89.5,
        },
        LeaderboardEntry {
            rank: 4,
            player_id: "player-4".to_string(),
            player_name: "Diana".to_string(),
            country: "DE".to_string(),
            cities_completed: 1,
            total_distance: 45.2,
        },
    ];

    HttpResponse::Ok().json(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn test_get_leaderboard() {
        let response = get_leaderboard().await;
        assert_eq!(response.status(), 200);
    }
}
