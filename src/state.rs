//! State bersama yang dibagikan ke semua handler via Axum `State`.

use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}
