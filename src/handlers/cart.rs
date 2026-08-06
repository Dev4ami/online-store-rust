//! Handler keranjang belanja (session-backed, interaksi HTMX).

use askama::Template;
use axum::Form;
use axum::extract::State;
use axum::response::Html;
use serde::Deserialize;
use uuid::Uuid;

use crate::cart::Cart;
use crate::error::AppError;
use crate::state::AppState;
use crate::templates::{CartContentsTemplate, CartTemplate};
use tower_sessions::Session;

#[derive(Deserialize)]
pub struct AddForm {
    pub product_id: Uuid,
    #[serde(default = "default_qty")]
    pub qty: i32,
}

fn default_qty() -> i32 {
    1
}

#[derive(Deserialize)]
pub struct UpdateForm {
    pub product_id: Uuid,
    pub qty: i32,
}

#[derive(Deserialize)]
pub struct RemoveForm {
    pub product_id: Uuid,
}

/// GET /cart — halaman keranjang penuh.
pub async fn view(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let mut cart = Cart::load(&session).await;
    let (lines, total) = cart.detailed(&state.pool).await?;
    // Simpan lagi kalau ada item ter-prune.
    cart.save(&session).await.map_err(|_| AppError::NotFound)?;

    let html = CartTemplate {
        cart_count: cart.total_qty(),
        grand_total: Cart::grand_total_display(total),
        lines,
    }
    .render()?;
    Ok(Html(html))
}

/// POST /cart/add — tambah produk ke keranjang. Balikan: partial isi cart (badge OOB ikut).
pub async fn add(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<AddForm>,
) -> Result<Html<String>, AppError> {
    // Validasi produk ada & aktif sebelum masuk cart.
    let product = crate::models::product::Product::by_id(&state.pool, form.product_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut cart = Cart::load(&session).await;
    cart.add(product.id, form.qty);
    cart.save(&session).await.map_err(|_| AppError::NotFound)?;

    render_contents(&state, &session, &mut cart).await
}

/// POST /cart/update — set qty absolut (0 = hapus).
pub async fn update(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<UpdateForm>,
) -> Result<Html<String>, AppError> {
    let mut cart = Cart::load(&session).await;
    cart.set(form.product_id, form.qty);
    cart.save(&session).await.map_err(|_| AppError::NotFound)?;

    render_contents(&state, &session, &mut cart).await
}

/// POST /cart/remove — hapus produk dari keranjang.
pub async fn remove(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<RemoveForm>,
) -> Result<Html<String>, AppError> {
    let mut cart = Cart::load(&session).await;
    cart.remove(form.product_id);
    cart.save(&session).await.map_err(|_| AppError::NotFound)?;

    render_contents(&state, &session, &mut cart).await
}

/// Render partial isi cart (dipakai add/update/remove).
async fn render_contents(
    state: &AppState,
    session: &Session,
    cart: &mut Cart,
) -> Result<Html<String>, AppError> {
    let (lines, total) = cart.detailed(&state.pool).await?;
    cart.save(session).await.map_err(|_| AppError::NotFound)?;
    let html = CartContentsTemplate::new(lines, total, cart.total_qty()).render()?;
    Ok(Html(html))
}
