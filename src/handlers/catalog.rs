//! Handler katalog: daftar produk, detail, health check.

use askama::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use tower_sessions::Session;

use crate::cart::Cart;
use crate::error::AppError;
use crate::models::product::Product;
use crate::state::AppState;
use crate::templates::{IndexTemplate, ProductTemplate};

/// GET / — daftar produk aktif.
pub async fn index(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let cart_count = Cart::load(&session).await.total_qty();
    let products = Product::list_active(&state.pool).await?;
    let html = IndexTemplate { products, cart_count }.render()?;
    Ok(Html(html))
}

/// GET /product/{slug} — detail satu produk.
pub async fn detail(
    State(state): State<AppState>,
    session: Session,
    Path(slug): Path<String>,
) -> Result<Html<String>, AppError> {
    let cart_count = Cart::load(&session).await.total_qty();
    let product = Product::by_slug(&state.pool, &slug)
        .await?
        .ok_or(AppError::NotFound)?;
    let html = ProductTemplate { product, cart_count }.render()?;
    Ok(Html(html))
}

/// GET /health — cek server hidup.
pub async fn health() -> &'static str {
    "ok"
}
