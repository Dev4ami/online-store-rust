mod auth;
mod cart;
mod config;
mod csrf;
mod db;
mod error;
mod handlers;
mod models;
mod payment;
mod ratelimit;
mod state;
mod templates;
mod uploads;

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_sessions::cookie::SameSite;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::models::user::{AdminSeed, User};
use crate::payment::PaymentGateway;
use crate::payment::dummy::DummyGateway;
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
    // Nama toko untuk brand/title (dipakai template via templates::store_name()).
    templates::set_store_name(config.store_name.clone());
    tracing::info!("menghubungkan ke database & menjalankan migrasi...");
    let pool = db::connect_and_migrate(&config.database_url).await?;

    // Pastikan akun admin dari env (idempoten). Bila ADMIN_PASSWORD diset & email
    // belum terdaftar, akun admin dibuat otomatis → deploy toko baru langsung siap.
    if let Some(email) = &config.admin_email {
        // Hash sandi seed di luar query agar error hashing terpisah dari error DB.
        let hash = match &config.admin_password {
            Some(pw) => match auth::hash_password(pw) {
                Ok(h) => Some(h),
                Err(_) => {
                    tracing::warn!("gagal hash ADMIN_PASSWORD; admin tak di-seed");
                    None
                }
            },
            None => None,
        };
        match User::ensure_admin(&pool, email, hash.as_deref()).await {
            Ok(AdminSeed::Created) => tracing::info!("akun admin dibuat dari env: {email}"),
            Ok(AdminSeed::Promoted) => tracing::info!("akun dipromosikan admin: {email}"),
            Ok(AdminSeed::AlreadyAdmin) => {}
            Ok(AdminSeed::Skipped) => tracing::warn!(
                "ADMIN_EMAIL {email} belum terdaftar & ADMIN_PASSWORD kosong — admin tak dibuat"
            ),
            Err(e) => tracing::warn!("gagal set admin {email}: {e}"),
        }
    }

    // Session store di Postgres (persist lintas restart).
    let session_store = PostgresStore::new(pool.clone());
    session_store.migrate().await?;
    let session_layer = SessionManagerLayer::new(session_store)
        // Dev (HTTP) default false; produksi set SECURE_COOKIES=true (butuh HTTPS).
        .with_secure(config.secure_cookies)
        // Lax: cookie tetap ikut pada navigasi top-level dari luar (link masuk
        // tetap login) TAPI tidak pada POST lintas-situs — lapis pertama anti-CSRF,
        // dilengkapi token per-form (lihat `csrf`). Eksplisit agar tak bergantung default dep.
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(time::Duration::days(30)));

    // Gateway pembayaran. Dummy untuk dev; ganti implementor saat gateway nyata dipilih.
    let gateway: Arc<dyn PaymentGateway> =
        Arc::new(DummyGateway::new(config.payment_dummy_secret.clone()));

    let state = AppState {
        pool,
        gateway,
        dummy_secret: config.payment_dummy_secret.clone(),
        // Maks 5 percobaan login/registrasi per IP tiap 5 menit.
        login_limiter: Arc::new(ratelimit::RateLimiter::new(
            5,
            std::time::Duration::from_secs(300),
        )),
        trust_proxy: config.trust_proxy,
    };

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
        .route("/checkout", get(handlers::order::checkout_form).post(handlers::order::checkout))
        .route("/order/{id}", get(handlers::order::detail))
        .route("/orders", get(handlers::order::list))
        .route("/pay/{id}", get(handlers::payment::pay_page))
        .route("/webhook/payment", post(handlers::payment::webhook))
        .route("/admin", get(handlers::admin::dashboard))
        .route(
            "/admin/products",
            get(handlers::admin::products)
                .post(handlers::admin::product_create)
                // Foto bisa > 2 MB (default) sebelum dikompres; naikkan khusus route ini.
                .layer(DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
        .route("/admin/products/new", get(handlers::admin::product_new))
        .route(
            "/admin/products/{id}/edit",
            get(handlers::admin::product_edit),
        )
        .route(
            "/admin/products/{id}",
            post(handlers::admin::product_update)
                .layer(DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
        .route(
            "/admin/products/{id}/delete",
            post(handlers::admin::product_delete),
        )
        .route("/admin/orders", get(handlers::admin::orders))
        .route("/admin/orders/{id}", get(handlers::admin::order_detail))
        .route("/admin/orders/{id}/pay", post(handlers::admin::order_pay))
        .route(
            "/admin/orders/{id}/cancel",
            post(handlers::admin::order_cancel),
        )
        .nest_service("/static", ServeDir::new("static"))
        // CSRF di dalam session_layer: Session harus sudah ada di extensions saat
        // middleware jalan. `session_layer` ditambah setelahnya = lapisan luar.
        .layer(axum::middleware::from_fn(csrf::middleware))
        .layer(session_layer)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!("server berjalan di http://{}", config.bind_addr);
    // `into_make_service_with_connect_info` agar handler bisa ekstrak
    // `ConnectInfo<SocketAddr>` (dipakai rate-limit login per-IP).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
