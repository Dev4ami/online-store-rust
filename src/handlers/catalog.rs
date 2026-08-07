//! Handler katalog: daftar produk, pencarian/filter, detail, health check.

use std::str::FromStr;

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Html;
use rust_decimal::Decimal;
use serde::Deserialize;
use tower_sessions::Session;

use crate::auth;
use crate::cart::Cart;
use crate::error::AppError;
use crate::models::product::{Product, ProductSort};
use crate::state::AppState;
use crate::templates::{CatalogGridTemplate, IndexTemplate, ProductTemplate};

/// Parameter pencarian/filter katalog dari query string.
#[derive(Debug, Default, Deserialize)]
pub struct CatalogQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub min: Option<String>,
    #[serde(default)]
    pub max: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
}

/// Parse string harga → Decimal; kosong/invalid → None.
fn parse_price(raw: &Option<String>) -> Option<Decimal> {
    let s = raw.as_deref()?.trim();
    if s.is_empty() {
        return None;
    }
    Decimal::from_str(s).ok()
}

/// GET / — daftar produk aktif, dengan pencarian/filter/urutan opsional.
/// Request HTMX (header `HX-Request`) → balas partial grid saja untuk di-swap.
pub async fn index(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Query(params): Query<CatalogQuery>,
) -> Result<Html<String>, AppError> {
    // Normalisasi input.
    let q_trimmed = params.q.as_deref().map(str::trim).unwrap_or("");
    let q_opt = if q_trimmed.is_empty() { None } else { Some(q_trimmed) };
    let min = parse_price(&params.min);
    let max = parse_price(&params.max);
    let sort = params
        .sort
        .as_deref()
        .map(ProductSort::from_query)
        .unwrap_or(ProductSort::Newest);

    let products = Product::search(&state.pool, q_opt, min, max, sort).await?;

    // Request HTMX → cukup kirim grid untuk di-swap ke #product-grid.
    if headers.contains_key("HX-Request") {
        let html = CatalogGridTemplate { products }.render()?;
        return Ok(Html(html));
    }

    // Full-load → halaman lengkap + echo nilai toolbar agar tetap terisi.
    let cart_count = Cart::load(&session).await.total_qty();
    let user_name = auth::current_user_name(&session, &state.pool).await;
    let html = IndexTemplate {
        products,
        cart_count,
        user_name,
        q: q_opt.unwrap_or("").to_string(),
        min: params.min.map(|s| s.trim().to_string()).unwrap_or_default(),
        max: params.max.map(|s| s.trim().to_string()).unwrap_or_default(),
        sort: sort.as_query().to_string(),
    }
    .render()?;
    Ok(Html(html))
}

/// GET /product/{slug} — detail satu produk.
pub async fn detail(
    State(state): State<AppState>,
    session: Session,
    Path(slug): Path<String>,
) -> Result<Html<String>, AppError> {
    let cart_count = Cart::load(&session).await.total_qty();
    let user_name = auth::current_user_name(&session, &state.pool).await;
    let product = Product::by_slug(&state.pool, &slug)
        .await?
        .ok_or(AppError::NotFound)?;
    let html = ProductTemplate { product, cart_count, user_name }.render()?;
    Ok(Html(html))
}

/// GET /health — cek server hidup.
pub async fn health() -> &'static str {
    "ok"
}
