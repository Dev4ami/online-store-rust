//! Handler autentikasi: registrasi, login, logout.

use std::net::SocketAddr;

use askama::Template;
use axum::Form;
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use tower_sessions::Session;

use crate::auth;
use crate::cart::Cart;
use crate::error::AppError;
use crate::models::user::User;
use crate::state::AppState;
use crate::templates::{LoginTemplate, RegisterTemplate};

#[derive(Deserialize)]
pub struct RegisterForm {
    pub email: String,
    pub name: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
}

/// Validasi email sangat sederhana (ada '@' dan '.').
fn email_valid(email: &str) -> bool {
    let e = email.trim();
    e.len() >= 5 && e.contains('@') && e.contains('.')
}

/// IP klien untuk rate-limit: hormati `X-Forwarded-For` hanya bila `trust_proxy`
/// (di belakang proxy tepercaya). Logika pemilihan ada di `ratelimit::client_ip`.
fn rate_limit_ip(headers: &HeaderMap, peer: std::net::IpAddr, trust_proxy: bool) -> std::net::IpAddr {
    let xff = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok());
    crate::ratelimit::client_ip(peer, xff, trust_proxy)
}

/// GET /register — tampilkan form.
pub async fn register_form(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let cart_count = Cart::load(&session).await.total_qty();
    let (user_name, is_admin) = auth::current_user_header(&session, &state.pool).await;
    let html = RegisterTemplate {
        cart_count,
        user_name,
        is_admin,
        error: None,
        email: String::new(),
        name: String::new(),
    }
    .render()?;
    Ok(Html(html))
}

/// POST /register — buat akun lalu login otomatis.
pub async fn register(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    session: Session,
    Form(form): Form<RegisterForm>,
) -> Result<Response, AppError> {
    let email = form.email.trim();
    let cart_count = Cart::load(&session).await.total_qty();
    let client_ip = rate_limit_ip(&headers, addr.ip(), state.trust_proxy);

    // Helper untuk render ulang form dengan pesan error.
    let render_err = |msg: &str| -> Result<Response, AppError> {
        let html = RegisterTemplate {
            cart_count,
            user_name: None,
            is_admin: false,
            error: Some(msg.to_string()),
            email: email.to_string(),
            name: form.name.clone(),
        }
        .render()?;
        Ok(Html(html).into_response())
    };

    if !state.login_limiter.check(client_ip) {
        return render_err("Terlalu banyak percobaan. Coba lagi beberapa menit.");
    }
    if !email_valid(email) {
        return render_err("Format email tidak valid.");
    }
    if form.password.len() < 8 {
        return render_err("Password minimal 8 karakter.");
    }
    if User::by_email(&state.pool, email).await?.is_some() {
        return render_err("Email sudah terdaftar. Silakan login.");
    }

    let hash = auth::hash_password(&form.password)
        .map_err(|_| AppError::Internal("gagal hash password".into()))?;
    let user = User::create(&state.pool, email, &hash, form.name.trim()).await?;

    auth::login(&session, user.id)
        .await
        .map_err(|_| AppError::Internal("gagal set session".into()))?;

    Ok(Redirect::to("/").into_response())
}

/// GET /login — tampilkan form.
pub async fn login_form(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let cart_count = Cart::load(&session).await.total_qty();
    let (user_name, is_admin) = auth::current_user_header(&session, &state.pool).await;
    let html = LoginTemplate {
        cart_count,
        user_name,
        is_admin,
        error: None,
        email: String::new(),
    }
    .render()?;
    Ok(Html(html))
}

/// POST /login — verifikasi kredensial.
pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    session: Session,
    Form(form): Form<LoginForm>,
) -> Result<Response, AppError> {
    let email = form.email.trim();
    let cart_count = Cart::load(&session).await.total_qty();
    let client_ip = rate_limit_ip(&headers, addr.ip(), state.trust_proxy);

    let render_err = |msg: &str| -> Result<Response, AppError> {
        let html = LoginTemplate {
            cart_count,
            user_name: None,
            is_admin: false,
            error: Some(msg.to_string()),
            email: email.to_string(),
        }
        .render()?;
        Ok(Html(html).into_response())
    };

    if !state.login_limiter.check(client_ip) {
        return render_err("Terlalu banyak percobaan. Coba lagi beberapa menit.");
    }

    // Pesan sama untuk email tak ada / password salah (hindari user enumeration).
    let user = match User::by_email(&state.pool, email).await? {
        Some(u) => u,
        None => return render_err("Email atau password salah."),
    };

    if !auth::verify_password(&form.password, &user.password_hash) {
        return render_err("Email atau password salah.");
    }

    auth::login(&session, user.id)
        .await
        .map_err(|_| AppError::Internal("gagal set session".into()))?;

    Ok(Redirect::to("/").into_response())
}

/// POST /logout — akhiri sesi login.
pub async fn logout(session: Session) -> Result<Response, AppError> {
    auth::logout(&session)
        .await
        .map_err(|_| AppError::Internal("gagal logout".into()))?;
    Ok(Redirect::to("/").into_response())
}
