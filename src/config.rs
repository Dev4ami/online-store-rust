//! Konfigurasi aplikasi dari environment variable.

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
}

impl Config {
    /// Baca konfigurasi dari env. `DATABASE_URL` wajib ada.
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL wajib diset (lihat .env.example)"))?;
        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
        Ok(Self { database_url, bind_addr })
    }
}
