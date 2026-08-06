//! Handler checkout & pesanan.

use askama::Template;
use axum::Form;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use tower_sessions::Session;
use uuid::Uuid;

use crate::auth;
use crate::cart::Cart;
use crate::error::AppError;
use crate::models::order::{CheckoutError, CheckoutInfo, Order};
use crate::models::user::User;
use crate::state::AppState;
use crate::templates::{CheckoutTemplate, OrderDetailTemplate, OrdersTemplate};

#[derive(Deserialize)]
pub struct CheckoutForm {
    pub email: String,
    pub customer_name: String,
    pub phone: String,
    pub shipping_address: String,
}

/// GET /checkout — form data pengiriman + ringkasan keranjang.
pub async fn checkout_form(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, AppError> {
    let mut cart = Cart::load(&session).await;
    let (lines, total) = cart.detailed(&state.pool).await?;
    cart.save(&session).await.map_err(|_| AppError::NotFound)?;

    // Keranjang kosong -> tak ada yang di-checkout.
    if lines.is_empty() {
        return Ok(Redirect::to("/cart").into_response());
    }

    // Prefill dari akun bila login.
    let (mut email, mut name) = (String::new(), String::new());
    if let Some(uid) = auth::current_user_id(&session).await {
        if let Some(u) = User::by_id(&state.pool, uid).await? {
            email = u.email;
            name = u.name;
        }
    }
    let user_name = auth::current_user_name(&session, &state.pool).await;

    let html = CheckoutTemplate {
        cart_count: cart.total_qty(),
        user_name,
        lines,
        grand_total: Cart::grand_total_display(total),
        error: None,
        email,
        customer_name: name,
        phone: String::new(),
        shipping_address: String::new(),
    }
    .render()?;
    Ok(Html(html).into_response())
}

/// POST /checkout — buat order dari keranjang.
pub async fn checkout(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<CheckoutForm>,
) -> Result<Response, AppError> {
    let mut cart = Cart::load(&session).await;
    let (lines, total) = cart.detailed(&state.pool).await?;
    cart.save(&session).await.map_err(|_| AppError::NotFound)?;

    if lines.is_empty() {
        return Ok(Redirect::to("/cart").into_response());
    }

    let email = form.email.trim().to_string();
    let name = form.customer_name.trim().to_string();
    let phone = form.phone.trim().to_string();
    let address = form.shipping_address.trim().to_string();
    let cart_count = cart.total_qty();
    let grand_total = Cart::grand_total_display(total);
    let user_name = auth::current_user_name(&session, &state.pool).await;

    // Bangun ulang halaman checkout dengan pesan error (ringkasan cart tetap tampil).
    let render_err = |msg: String| -> Result<Response, AppError> {
        let html = CheckoutTemplate {
            cart_count,
            user_name: user_name.clone(),
            lines: lines.clone(),
            grand_total: grand_total.clone(),
            error: Some(msg),
            email: email.clone(),
            customer_name: name.clone(),
            phone: phone.clone(),
            shipping_address: address.clone(),
        }
        .render()?;
        Ok(Html(html).into_response())
    };

    if email.len() < 5 || !email.contains('@') || !email.contains('.') {
        return render_err("Email tidak valid.".into());
    }
    if name.is_empty() {
        return render_err("Nama penerima wajib diisi.".into());
    }
    if address.is_empty() {
        return render_err("Alamat pengiriman wajib diisi.".into());
    }

    let user_id = auth::current_user_id(&session).await;
    let info = CheckoutInfo {
        email: email.clone(),
        customer_name: name.clone(),
        phone: phone.clone(),
        shipping_address: address.clone(),
    };
    let items: Vec<(Uuid, i32)> = cart.items.iter().map(|(id, q)| (*id, *q)).collect();

    match Order::create_from_cart(&state.pool, user_id, &info, &items).await {
        Ok(order) => {
            // Kosongkan keranjang & ingat order agar konfirmasi bisa diakses.
            cart.items.clear();
            cart.save(&session).await.map_err(|_| AppError::NotFound)?;
            auth::remember_order(&session, order.id).await;
            Ok(Redirect::to(&format!("/order/{}", order.id)).into_response())
        }
        Err(CheckoutError::EmptyCart) => Ok(Redirect::to("/cart").into_response()),
        Err(CheckoutError::OutOfStock(nama)) => {
            render_err(format!("Stok tidak cukup untuk: {nama}. Kurangi jumlah."))
        }
        Err(CheckoutError::Db(e)) => Err(AppError::Database(e)),
    }
}

/// GET /order/{id} — halaman konfirmasi/detail order.
/// Akses: pemilik (login) ATAU order tercatat di session (guest).
pub async fn detail(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, AppError> {
    let order = Order::by_id(&state.pool, id).await?.ok_or(AppError::NotFound)?;

    // Kontrol akses.
    let current = auth::current_user_id(&session).await;
    let is_owner = matches!((order.user_id, current), (Some(o), Some(c)) if o == c);
    if !is_owner && !auth::remembers_order(&session, id).await {
        return Err(AppError::NotFound);
    }

    let items = Order::items(&state.pool, id).await?;
    let cart_count = Cart::load(&session).await.total_qty();
    let user_name = auth::current_user_name(&session, &state.pool).await;

    let html = OrderDetailTemplate {
        cart_count,
        user_name,
        order,
        items,
    }
    .render()?;
    Ok(Html(html))
}

/// GET /orders — riwayat pesanan (wajib login).
pub async fn list(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, AppError> {
    let Some(uid) = auth::current_user_id(&session).await else {
        return Ok(Redirect::to("/login").into_response());
    };

    let orders = Order::list_for_user(&state.pool, uid).await?;
    let cart_count = Cart::load(&session).await.total_qty();
    let user_name = auth::current_user_name(&session, &state.pool).await;

    let html = OrdersTemplate {
        cart_count,
        user_name,
        orders,
    }
    .render()?;
    Ok(Html(html).into_response())
}
