mod config;
mod db;
mod error;
mod handlers;
mod models;
mod state;
mod templates;

use axum::Router;
use axum::routing::get;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env (abaikan bila tidak ada — env asli tetap dipakai di produksi).
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    tracing::info!("menghubungkan ke database & menjalankan migrasi...");
    let pool = db::connect_and_migrate(&config.database_url).await?;

    let state = AppState { pool };

    let app = Router::new()
        .route("/", get(handlers::catalog::index))
        .route("/product/{slug}", get(handlers::catalog::detail))
        .route("/health", get(handlers::catalog::health))
        .nest_service("/static", ServeDir::new("static"))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!("server berjalan di http://{}", config.bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
