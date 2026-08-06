//! Handler katalog: daftar produk, detail, health check.

use askama::Template;
use axum::extract::{Path, State};
use axum::response::Html;

use crate::error::AppError;
use crate::models::product::Product;
use crate::state::AppState;
use crate::templates::{IndexTemplate, ProductTemplate};

/// GET / — daftar produk aktif.
pub async fn index(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let products = Product::list_active(&state.pool).await?;
    let html = IndexTemplate { products }.render()?;
    Ok(Html(html))
}

/// GET /product/{slug} — detail satu produk.
pub async fn detail(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Html<String>, AppError> {
    let product = Product::by_slug(&state.pool, &slug)
        .await?
        .ok_or(AppError::NotFound)?;
    let html = ProductTemplate { product }.render()?;
    Ok(Html(html))
}

/// GET /health — cek server hidup.
pub async fn health() -> &'static str {
    "ok"
}
