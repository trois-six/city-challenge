use crate::models::{City, Comment, CommentRequest, PathData, PlayerResult};
use actix_web::{web, HttpResponse};
use chrono::Utc;
use uuid::Uuid;

pub async fn list_cities() -> HttpResponse {
    // Mock data for now
    let cities = vec![
        City {
            id: "FR-75-75001-Paris".to_string(),
            country: "FR".to_string(),
            region: "Île-de-France".to_string(),
            department: "75".to_string(),
            postal_code: "75001".to_string(),
            name: "Paris".to_string(),
            date: "2026-06-14".to_string(),
            street_count: 8234,
            total_meters: 1234567,
        },
        City {
            id: "FR-69-69001-Lyon".to_string(),
            country: "FR".to_string(),
            region: "Auvergne-Rhône-Alpes".to_string(),
            department: "69".to_string(),
            postal_code: "69001".to_string(),
            name: "Lyon".to_string(),
            date: "2026-06-14".to_string(),
            street_count: 5123,
            total_meters: 856432,
        },
    ];

    HttpResponse::Ok().json(cities)
}

pub async fn get_city(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();

    let city = City {
        id: id.clone(),
        country: "FR".to_string(),
        region: "Île-de-France".to_string(),
        department: "75".to_string(),
        postal_code: "75001".to_string(),
        name: "Paris".to_string(),
        date: "2026-06-14".to_string(),
        street_count: 8234,
        total_meters: 1234567,
    };

    HttpResponse::Ok().json(city)
}

pub async fn get_leaderboard(path: web::Path<String>) -> HttpResponse {
    let _city_id = path.into_inner();

    let results = vec![
        PlayerResult {
            player_id: "player-1".to_string(),
            player_name: "Alice".to_string(),
            country: "FR".to_string(),
            city_id: "FR-75-75001-Paris".to_string(),
            distance: 28.5,
            completed: true,
            rank: 1,
        },
        PlayerResult {
            player_id: "player-2".to_string(),
            player_name: "Bob".to_string(),
            country: "FR".to_string(),
            city_id: "FR-75-75001-Paris".to_string(),
            distance: 24.3,
            completed: true,
            rank: 2,
        },
        PlayerResult {
            player_id: "player-3".to_string(),
            player_name: "Charlie".to_string(),
            country: "FR".to_string(),
            city_id: "FR-75-75001-Paris".to_string(),
            distance: 15.7,
            completed: false,
            rank: 3,
        },
    ];

    HttpResponse::Ok().json(results)
}

pub async fn get_path(path: web::Path<String>) -> HttpResponse {
    let city_id = path.into_inner();

    let path_data = PathData {
        id: city_id.clone(),
        city_id: city_id.clone(),
        coordinates: vec![
            [48.8566, 2.3522],
            [48.8600, 2.3500],
            [48.8550, 2.3550],
            [48.8566, 2.3522],
        ],
        total_distance: 12.5,
        street_count: 234,
    };

    HttpResponse::Ok().json(path_data)
}

pub async fn post_comment(path: web::Path<String>, req: web::Json<CommentRequest>) -> HttpResponse {
    let city_id = path.into_inner();

    let comment = Comment {
        id: Uuid::new_v4().to_string(),
        city_id,
        text: req.text.clone(),
        created_at: Utc::now().to_rfc3339(),
    };

    HttpResponse::Created().json(comment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn test_list_cities() {
        let response = list_cities().await;
        assert_eq!(response.status(), 200);
    }

    #[actix_web::test]
    async fn test_get_city() {
        let response = get_city(web::Path::from("FR-75-75001-Paris".to_string())).await;
        assert_eq!(response.status(), 200);
    }

    #[actix_web::test]
    async fn test_get_leaderboard() {
        let response = get_leaderboard(web::Path::from("FR-75-75001-Paris".to_string())).await;
        assert_eq!(response.status(), 200);
    }

    #[actix_web::test]
    async fn test_get_path() {
        let response = get_path(web::Path::from("FR-75-75001-Paris".to_string())).await;
        assert_eq!(response.status(), 200);
    }
}
