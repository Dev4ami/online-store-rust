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
use crate::models::product::{PER_PAGE, Product, ProductSort};
use crate::state::AppState;
use crate::templates::{CatalogGridTemplate, IndexTemplate, Pagination, ProductTemplate, page_window};

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
    #[serde(default)]
    pub page: Option<String>,
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
    // Halaman diminta (minimal 1); nilai jorok → 1.
    let req_page = params
        .page
        .as_deref()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(1)
        .max(1);

    // Ambil dulu untuk tahu total; clamp page bila melewati batas, lalu ambil halaman benar.
    let mut offset = (req_page - 1) * PER_PAGE;
    let mut result = Product::search(&state.pool, q_opt, min, max, sort, PER_PAGE, offset).await?;
    let total_pages = ((result.total + PER_PAGE - 1) / PER_PAGE).max(1);
    let mut page = req_page;
    if page > total_pages {
        page = total_pages;
        offset = (page - 1) * PER_PAGE;
        result = Product::search(&state.pool, q_opt, min, max, sort, PER_PAGE, offset).await?;
    }

    let pg = Pagination {
        q: q_opt.unwrap_or("").to_string(),
        min: params.min.as_deref().map(str::trim).unwrap_or("").to_string(),
        max: params.max.as_deref().map(str::trim).unwrap_or("").to_string(),
        sort: sort.as_query().to_string(),
        page,
        total_pages,
        pages: page_window(page, total_pages),
        has_prev: page > 1,
        has_next: page < total_pages,
    };
    let products = result.items;

    // Request HTMX → cukup kirim grid + nav untuk di-swap ke #catalog-results.
    if headers.contains_key("HX-Request") {
        let html = CatalogGridTemplate { products, pg }.render()?;
        return Ok(Html(html));
    }

    // Full-load → halaman lengkap.
    let cart_count = Cart::load(&session).await.total_qty();
    let (user_name, is_admin) = auth::current_user_header(&session, &state.pool).await;
    let html = IndexTemplate {
        products,
        cart_count,
        user_name,
        is_admin,
        pg,
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
    let (user_name, is_admin) = auth::current_user_header(&session, &state.pool).await;
    let product = Product::by_slug(&state.pool, &slug)
        .await?
        .ok_or(AppError::NotFound)?;
    let html = ProductTemplate { product, cart_count, user_name, is_admin }.render()?;
    Ok(Html(html))
}

/// GET /health — cek server hidup.
pub async fn health() -> &'static str {
    "ok"
}
