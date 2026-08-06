//! Panel admin: CRUD produk + kelola pesanan.
//!
//! Semua handler dijaga `auth::current_admin`; non-admin dapat 404 (sembunyikan panel).

use std::str::FromStr;

use askama::Template;
use axum::Form;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use rust_decimal::Decimal;
use serde::Deserialize;
use tower_sessions::Session;
use uuid::Uuid;

use crate::auth;
use crate::error::AppError;
use crate::models::order::Order;
use crate::models::product::{NewProduct, Product};
use crate::models::user::User;
use crate::state::AppState;
use crate::templates::{
    AdminOrderDetailTemplate, AdminOrdersTemplate, AdminProductFormTemplate, AdminProductsTemplate,
};

/// Nama tampilan admin (fallback ke email bila nama kosong).
fn admin_display_name(admin: &User) -> String {
    if admin.name.is_empty() {
        admin.email.clone()
    } else {
        admin.name.clone()
    }
}

/// Ambil admin dari session atau 404. Dipakai di awal tiap handler.
async fn require_admin(session: &Session, state: &AppState) -> Result<User, AppError> {
    auth::current_admin(session, &state.pool)
        .await
        .ok_or(AppError::NotFound)
}

/// GET /admin — arahkan ke daftar produk.
pub async fn dashboard(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, AppError> {
    require_admin(&session, &state).await?;
    Ok(Redirect::to("/admin/products").into_response())
}

// ----- Produk -----

/// GET /admin/products — daftar semua produk.
pub async fn products(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let admin = require_admin(&session, &state).await?;
    let products = Product::list_all(&state.pool).await?;
    let html = AdminProductsTemplate {
        admin_name: admin_display_name(&admin),
        products,
    }
    .render()?;
    Ok(Html(html))
}

/// GET /admin/products/new — form tambah produk kosong.
pub async fn product_new(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let admin = require_admin(&session, &state).await?;
    let html = AdminProductFormTemplate {
        admin_name: admin_display_name(&admin),
        error: None,
        editing: false,
        action_url: "/admin/products".to_string(),
        slug: String::new(),
        name: String::new(),
        description: String::new(),
        price: String::new(),
        stock: String::new(),
        image_url: String::new(),
        is_active: true,
    }
    .render()?;
    Ok(Html(html))
}

/// Field form produk (mentah; price/stock diparse manual agar bisa balas pesan ramah).
#[derive(Deserialize)]
pub struct ProductForm {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub price: String,
    pub stock: String,
    #[serde(default)]
    pub image_url: String,
    // Checkbox: hadir ("on") bila dicentang, absen bila tidak.
    pub is_active: Option<String>,
}

/// Hasil validasi form: NewProduct siap simpan, atau pesan error.
fn validate_product(form: &ProductForm) -> Result<NewProduct, String> {
    let slug = form.slug.trim().to_string();
    let name = form.name.trim().to_string();
    let description = form.description.trim().to_string();
    let image_raw = form.image_url.trim().to_string();

    if name.is_empty() {
        return Err("Nama produk wajib diisi.".into());
    }
    if slug.is_empty() {
        return Err("Slug wajib diisi.".into());
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err("Slug hanya boleh huruf/angka/tanda hubung (-).".into());
    }

    let price = Decimal::from_str(form.price.trim())
        .map_err(|_| "Harga harus berupa angka (mis. 85000).".to_string())?;
    if price.is_sign_negative() {
        return Err("Harga tidak boleh negatif.".into());
    }

    let stock: i32 = form
        .stock
        .trim()
        .parse()
        .map_err(|_| "Stok harus berupa bilangan bulat.".to_string())?;
    if stock < 0 {
        return Err("Stok tidak boleh negatif.".into());
    }

    let image_url = if image_raw.is_empty() {
        None
    } else {
        Some(image_raw)
    };

    Ok(NewProduct {
        slug,
        name,
        description,
        price,
        stock,
        image_url,
        is_active: form.is_active.is_some(),
    })
}

/// Bangun ulang form dengan pesan error + input yang tadi diisi.
fn render_product_form(
    admin: &User,
    error: String,
    editing: bool,
    action_url: String,
    form: &ProductForm,
) -> Result<Response, AppError> {
    let html = AdminProductFormTemplate {
        admin_name: admin_display_name(admin),
        error: Some(error),
        editing,
        action_url,
        slug: form.slug.clone(),
        name: form.name.clone(),
        description: form.description.clone(),
        price: form.price.clone(),
        stock: form.stock.clone(),
        image_url: form.image_url.clone(),
        is_active: form.is_active.is_some(),
    }
    .render()?;
    Ok(Html(html).into_response())
}

/// POST /admin/products — buat produk baru.
pub async fn product_create(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<ProductForm>,
) -> Result<Response, AppError> {
    let admin = require_admin(&session, &state).await?;
    let action = "/admin/products".to_string();

    let new = match validate_product(&form) {
        Ok(n) => n,
        Err(msg) => return render_product_form(&admin, msg, false, action, &form),
    };
    if Product::slug_taken(&state.pool, &new.slug, None).await? {
        return render_product_form(&admin, "Slug sudah dipakai produk lain.".into(), false, action, &form);
    }

    Product::create(&state.pool, &new).await?;
    Ok(Redirect::to("/admin/products").into_response())
}

/// GET /admin/products/{id}/edit — form terisi.
pub async fn product_edit(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, AppError> {
    let admin = require_admin(&session, &state).await?;
    let p = Product::by_id_any(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let html = AdminProductFormTemplate {
        admin_name: admin_display_name(&admin),
        error: None,
        editing: true,
        action_url: format!("/admin/products/{id}"),
        slug: p.slug,
        name: p.name,
        description: p.description,
        price: p.price.to_string(),
        stock: p.stock.to_string(),
        image_url: p.image_url.unwrap_or_default(),
        is_active: p.is_active,
    }
    .render()?;
    Ok(Html(html))
}

/// POST /admin/products/{id} — simpan perubahan produk.
pub async fn product_update(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<Uuid>,
    Form(form): Form<ProductForm>,
) -> Result<Response, AppError> {
    let admin = require_admin(&session, &state).await?;
    let action = format!("/admin/products/{id}");

    let new = match validate_product(&form) {
        Ok(n) => n,
        Err(msg) => return render_product_form(&admin, msg, true, action, &form),
    };
    if Product::slug_taken(&state.pool, &new.slug, Some(id)).await? {
        return render_product_form(&admin, "Slug sudah dipakai produk lain.".into(), true, action, &form);
    }

    let updated = Product::update(&state.pool, id, &new).await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Redirect::to("/admin/products").into_response())
}

/// POST /admin/products/{id}/delete — hapus produk permanen.
pub async fn product_delete(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    require_admin(&session, &state).await?;
    Product::delete(&state.pool, id).await?;
    Ok(Redirect::to("/admin/products").into_response())
}

// ----- Pesanan -----

/// GET /admin/orders — daftar semua pesanan.
pub async fn orders(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let admin = require_admin(&session, &state).await?;
    let orders = Order::list_all(&state.pool).await?;
    let html = AdminOrdersTemplate {
        admin_name: admin_display_name(&admin),
        orders,
    }
    .render()?;
    Ok(Html(html))
}

/// GET /admin/orders/{id} — detail pesanan + aksi status.
pub async fn order_detail(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, AppError> {
    let admin = require_admin(&session, &state).await?;
    let order = Order::by_id(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let items = Order::items(&state.pool, id).await?;
    let html = AdminOrderDetailTemplate {
        admin_name: admin_display_name(&admin),
        order,
        items,
    }
    .render()?;
    Ok(Html(html))
}

/// POST /admin/orders/{id}/pay — tandai lunas manual (mis. transfer bank).
pub async fn order_pay(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    require_admin(&session, &state).await?;
    // Ambil nomor untuk referensi manual yang informatif.
    if let Some(order) = Order::by_id(&state.pool, id).await? {
        let reference = format!("ADMIN-{}", order.number);
        Order::mark_paid(&state.pool, id, "manual", &reference).await?;
    }
    Ok(Redirect::to(&format!("/admin/orders/{id}")).into_response())
}

/// POST /admin/orders/{id}/cancel — batalkan + kembalikan stok.
pub async fn order_cancel(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    require_admin(&session, &state).await?;
    Order::cancel(&state.pool, id).await?;
    Ok(Redirect::to(&format!("/admin/orders/{id}")).into_response())
}
