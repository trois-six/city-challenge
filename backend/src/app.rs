//! Application state and route configuration, shared between the `server`
//! binary and integration tests.

use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use std::sync::Arc;

use crate::handlers;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: Arc<Option<PgPool>>,
}

/// Register the `/api` routes on an Actix `App`/`ServiceConfig`.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .route("/health", web::get().to(health_check))
            .route("/cities", web::get().to(handlers::cities::list_cities))
            .route("/cities/{id}", web::get().to(handlers::cities::get_city))
            .route(
                "/cities/{id}/leaderboard",
                web::get().to(handlers::cities::get_leaderboard),
            )
            .route(
                "/cities/{id}/path",
                web::get().to(handlers::cities::get_path),
            )
            .route(
                "/cities/{id}/comments",
                web::post().to(handlers::cities::post_comment),
            )
            .route("/players", web::get().to(handlers::players::list_players))
            .route(
                "/players/{id}",
                web::get().to(handlers::players::get_player),
            )
            .route(
                "/leaderboard",
                web::get().to(handlers::leaderboard::get_leaderboard),
            ),
    );
}

pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "message": "City Challenge API is running"
    }))
}
