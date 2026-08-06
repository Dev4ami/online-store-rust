//! Koneksi database dan migrasi.

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

/// Buat connection pool ke Postgres dan jalankan migrasi yang tertunda.
pub async fn connect_and_migrate(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
