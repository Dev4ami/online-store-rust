//! State bersama yang dibagikan ke semua handler via Axum `State`.

use std::sync::Arc;

use sqlx::PgPool;

use crate::payment::PaymentGateway;
use crate::ratelimit::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    /// Gateway pembayaran aktif (abstrak). Dummy saat dev, konkret di produksi.
    pub gateway: Arc<dyn PaymentGateway>,
    /// Secret gateway dummy — hanya dipakai halaman bayar dev untuk mensimulasikan
    /// webhook bertanda tangan. Kosong/diabaikan saat gateway nyata dipakai.
    pub dummy_secret: String,
    /// Pembatas laju percobaan login/registrasi per-IP (anti brute-force).
    pub login_limiter: Arc<RateLimiter>,
}
