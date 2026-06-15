//! Integration tests for the Actix Web API, exercising the full
//! app/router/handler stack with `actix_web::test`. The database pool is
//! mocked out (`db_pool: None`) since handlers currently serve static data.

use std::sync::Arc;

use actix_web::{http::StatusCode, test, web, App};
use city_challenge_backend::app::{self, AppState};
use city_challenge_backend::models::{
    City, Comment, LeaderboardEntry, PathData, Player, PlayerResult,
};

fn test_app_state() -> AppState {
    AppState {
        db_pool: Arc::new(None),
    }
}

#[actix_web::test]
async fn health_check_reports_ok() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(test_app_state()))
            .configure(app::configure),
    )
    .await;

    let req = test::TestRequest::get().uri("/api/health").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "ok");
}

#[actix_web::test]
async fn list_cities_returns_cities() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(test_app_state()))
            .configure(app::configure),
    )
    .await;

    let req = test::TestRequest::get().uri("/api/cities").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let cities: Vec<City> = test::read_body_json(resp).await;
    assert!(!cities.is_empty());
}

#[actix_web::test]
async fn get_city_returns_requested_id() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(test_app_state()))
            .configure(app::configure),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/cities/FR-75-75001-Paris")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let city: City = test::read_body_json(resp).await;
    assert_eq!(city.id, "FR-75-75001-Paris");
}

#[actix_web::test]
async fn get_city_leaderboard_returns_results() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(test_app_state()))
            .configure(app::configure),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/cities/FR-75-75001-Paris/leaderboard")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let results: Vec<PlayerResult> = test::read_body_json(resp).await;
    assert!(!results.is_empty());
}

#[actix_web::test]
async fn get_city_path_returns_route() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(test_app_state()))
            .configure(app::configure),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/cities/FR-75-75001-Paris/path")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let path: PathData = test::read_body_json(resp).await;
    assert!(!path.coordinates.is_empty());
}

#[actix_web::test]
async fn post_comment_returns_created_comment() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(test_app_state()))
            .configure(app::configure),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/api/cities/FR-75-75001-Paris/comments")
        .set_json(serde_json::json!({ "text": "Great run!" }))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::CREATED);
    let comment: Comment = test::read_body_json(resp).await;
    assert_eq!(comment.city_id, "FR-75-75001-Paris");
    assert_eq!(comment.text, "Great run!");
}

#[actix_web::test]
async fn list_players_returns_players() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(test_app_state()))
            .configure(app::configure),
    )
    .await;

    let req = test::TestRequest::get().uri("/api/players").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let players: Vec<Player> = test::read_body_json(resp).await;
    assert!(!players.is_empty());
}

#[actix_web::test]
async fn get_player_returns_player() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(test_app_state()))
            .configure(app::configure),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/players/player-1")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let player: Player = test::read_body_json(resp).await;
    assert_eq!(player.id, "player-1");
}

#[actix_web::test]
async fn get_global_leaderboard_returns_entries() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(test_app_state()))
            .configure(app::configure),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/leaderboard")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let entries: Vec<LeaderboardEntry> = test::read_body_json(resp).await;
    assert!(!entries.is_empty());
}
