//! Koneksi database dan migrasi.

use sqlx::Postgres;
use sqlx::migrate::MigrateDatabase;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

/// Buat connection pool ke Postgres dan jalankan migrasi yang tertunda.
/// Bila database target belum ada, dibuat otomatis dulu (lihat
/// `ensure_database_exists`) agar sekali deploy langsung jalan.
pub async fn connect_and_migrate(database_url: &str) -> anyhow::Result<PgPool> {
    ensure_database_exists(database_url).await?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

/// Buat database target bila belum ada, agar deploy toko baru cukup mengisi
/// `DATABASE_URL` tanpa `CREATE DATABASE` manual. Idempoten & aman pada boot
/// berulang: bila database sudah ada (mis. dibuat manual), langkah dilewati.
///
/// Butuh: user DB berhak `CREATEDB` dan bisa akses database maintenance
/// (`postgres`). Pada Postgres yang dibuat Coolify, user default punya hak ini.
async fn ensure_database_exists(database_url: &str) -> anyhow::Result<()> {
    if Postgres::database_exists(database_url).await? {
        return Ok(());
    }
    tracing::info!("database target belum ada — membuat otomatis...");
    match Postgres::create_database(database_url).await {
        Ok(()) => {
            tracing::info!("database berhasil dibuat");
            Ok(())
        }
        // Instance lain mungkin membuatnya lebih dulu (boot paralel) → anggap sukses.
        Err(_) if Postgres::database_exists(database_url).await.unwrap_or(false) => {
            tracing::info!("database sudah dibuat instance lain");
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!(
            "gagal membuat database otomatis: {e}. Pastikan user DB berhak CREATEDB \
             & bisa akses database 'postgres', atau buat database itu manual."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bukti auto-create nyata (butuh Postgres hidup). Diabaikan default; jalankan:
    /// `AUTOCREATE_TEST_URL=postgres://user:pass@host:5432/tmpdb cargo test db:: -- --ignored`
    /// URL harus menunjuk nama database yang BELUM ada — test membuat lalu men-drop-nya.
    #[tokio::test]
    #[ignore]
    async fn auto_creates_and_migrates() {
        let url = std::env::var("AUTOCREATE_TEST_URL")
            .expect("set AUTOCREATE_TEST_URL ke DB throwaway yang belum ada");

        // Bersihkan sisa run sebelumnya bila ada.
        if Postgres::database_exists(&url).await.unwrap() {
            Postgres::drop_database(&url).await.unwrap();
        }
        assert!(!Postgres::database_exists(&url).await.unwrap(), "prasyarat: DB belum ada");

        let pool = connect_and_migrate(&url).await.expect("boot harus buat DB + migrasi");

        // Tabel dari migrasi harus ada → auto-create + migrate benar-benar jalan.
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'products')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(exists, "tabel products harus ada setelah migrasi");

        pool.close().await;
        Postgres::drop_database(&url).await.unwrap(); // bersih-bersih
    }
}
