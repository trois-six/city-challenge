use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use city_challenge_backend::app::{self, AppState};
use city_challenge_backend::db;
use std::sync::Arc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let database_url = std::env::var("DATABASE_URL").ok();
    let db_pool = match &database_url {
        Some(url) => match db::init_database(url).await {
            Ok(pool) => Some(pool),
            Err(err) => {
                log::warn!("Failed to connect to database: {err}");
                None
            }
        },
        None => None,
    };

    let app_state = AppState {
        db_pool: Arc::new(db_pool),
    };

    log::info!("Starting City Challenge backend server...");

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(middleware::Logger::default())
            .wrap(cors)
            .configure(app::configure)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
