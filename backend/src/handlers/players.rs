use crate::models::Player;
use actix_web::{web, HttpResponse};

pub async fn list_players() -> HttpResponse {
    let players = vec![
        Player {
            id: "player-1".to_string(),
            name: "Alice".to_string(),
            country: "FR".to_string(),
            total_distance: 156.8,
            cities_completed: 5,
        },
        Player {
            id: "player-2".to_string(),
            name: "Bob".to_string(),
            country: "FR".to_string(),
            total_distance: 128.3,
            cities_completed: 3,
        },
        Player {
            id: "player-3".to_string(),
            name: "Charlie".to_string(),
            country: "BE".to_string(),
            total_distance: 89.5,
            cities_completed: 2,
        },
    ];

    HttpResponse::Ok().json(players)
}

pub async fn get_player(path: web::Path<String>) -> HttpResponse {
    let _player_id = path.into_inner();

    let player = Player {
        id: "player-1".to_string(),
        name: "Alice".to_string(),
        country: "FR".to_string(),
        total_distance: 156.8,
        cities_completed: 5,
    };

    HttpResponse::Ok().json(player)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn test_list_players() {
        let response = list_players().await;
        assert_eq!(response.status(), 200);
    }

    #[actix_web::test]
    async fn test_get_player() {
        let response = get_player(web::Path::from("player-1".to_string())).await;
        assert_eq!(response.status(), 200);
    }
}
