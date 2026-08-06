mod auth;
mod cart;
mod config;
mod db;
mod error;
mod handlers;
mod models;
mod state;
mod templates;

use axum::Router;
use axum::routing::{get, post};
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;
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

    // Session store di Postgres (persist lintas restart).
    let session_store = PostgresStore::new(pool.clone());
    session_store.migrate().await?;
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // set true di produksi (HTTPS)
        .with_expiry(Expiry::OnInactivity(time::Duration::days(30)));

    let state = AppState { pool };

    let app = Router::new()
        .route("/", get(handlers::catalog::index))
        .route("/product/{slug}", get(handlers::catalog::detail))
        .route("/health", get(handlers::catalog::health))
        .route("/cart", get(handlers::cart::view))
        .route("/cart/add", post(handlers::cart::add))
        .route("/cart/update", post(handlers::cart::update))
        .route("/cart/remove", post(handlers::cart::remove))
        .route("/register", get(handlers::auth::register_form).post(handlers::auth::register))
        .route("/login", get(handlers::auth::login_form).post(handlers::auth::login))
        .route("/logout", post(handlers::auth::logout))
        .nest_service("/static", ServeDir::new("static"))
        .layer(session_layer)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!("server berjalan di http://{}", config.bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
