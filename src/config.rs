//! Konfigurasi aplikasi dari environment variable.

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    /// Rahasia bersama untuk verifikasi webhook gateway dummy (dev).
    /// Ganti dengan secret gateway asli saat gateway nyata dipilih.
    pub payment_dummy_secret: String,
    /// Email akun admin yang dipastikan ada saat startup (opsional).
    pub admin_email: Option<String>,
    /// Sandi untuk seed akun admin baru bila `admin_email` belum terdaftar
    /// (opsional). Bila kosong, admin hanya dipromosikan dari akun yang sudah
    /// ada (tak bisa buat akun baru). Password akun lama tak pernah ditimpa.
    pub admin_password: Option<String>,
    /// Nama toko yang tampil di brand header, footer, dan judul tab. Default "Toko Online".
    pub store_name: String,
    /// Set `true` di produksi (HTTPS) agar cookie session hanya dikirim lewat TLS.
    /// Default `false` untuk dev lokal (HTTP) — kalau `true` di HTTP, cookie tak
    /// terkirim dan login putus. Aktifkan via `SECURE_COOKIES=true`.
    pub secure_cookies: bool,
    /// Set `true` bila di belakang reverse proxy tepercaya (mis. Traefik/Coolify)
    /// agar rate-limit login membaca IP klien asli dari `X-Forwarded-For`. Default
    /// `false`: pakai IP soket (langsung). JANGAN `true` bila app terekspos langsung
    /// ke internet — klien bisa memalsukan header & mengelabui rate-limit.
    pub trust_proxy: bool,
}

impl Config {
    /// Baca konfigurasi dari env. `DATABASE_URL` wajib ada.
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL wajib diset (lihat .env.example)"))?;
        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
        let payment_dummy_secret =
            env::var("PAYMENT_DUMMY_SECRET").unwrap_or_else(|_| "dev-dummy-secret".to_string());
        // Kosong dianggap tak diset (hindari promote email "" saat env ada tapi kosong).
        let admin_email = env::var("ADMIN_EMAIL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        // Sandi seed admin (jangan trim: sandi bisa sengaja pakai spasi tepi).
        let admin_password = env::var("ADMIN_PASSWORD").ok().filter(|s| !s.is_empty());
        let store_name = env::var("STORE_NAME")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Toko Online".to_string());
        // Aktif hanya bila diset persis "true" (case-insensitive). Selain itu false.
        let secure_cookies = env::var("SECURE_COOKIES")
            .map(|s| s.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let trust_proxy = env::var("TRUST_PROXY_HEADERS")
            .map(|s| s.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Ok(Self {
            database_url,
            bind_addr,
            payment_dummy_secret,
            admin_email,
            admin_password,
            store_name,
            secure_cookies,
            trust_proxy,
        })
    }
}
