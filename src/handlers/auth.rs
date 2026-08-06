//! Handler autentikasi: registrasi, login, logout.

use askama::Template;
use axum::Form;
use axum::extract::State;
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

/// GET /register — tampilkan form.
pub async fn register_form(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let cart_count = Cart::load(&session).await.total_qty();
    let user_name = auth::current_user_name(&session, &state.pool).await;
    let html = RegisterTemplate {
        cart_count,
        user_name,
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
    session: Session,
    Form(form): Form<RegisterForm>,
) -> Result<Response, AppError> {
    let email = form.email.trim();
    let cart_count = Cart::load(&session).await.total_qty();

    // Helper untuk render ulang form dengan pesan error.
    let render_err = |msg: &str| -> Result<Response, AppError> {
        let html = RegisterTemplate {
            cart_count,
            user_name: None,
            error: Some(msg.to_string()),
            email: email.to_string(),
            name: form.name.clone(),
        }
        .render()?;
        Ok(Html(html).into_response())
    };

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
    let user_name = auth::current_user_name(&session, &state.pool).await;
    let html = LoginTemplate {
        cart_count,
        user_name,
        error: None,
        email: String::new(),
    }
    .render()?;
    Ok(Html(html))
}

/// POST /login — verifikasi kredensial.
pub async fn login(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> Result<Response, AppError> {
    let email = form.email.trim();
    let cart_count = Cart::load(&session).await.total_qty();

    let render_err = |msg: &str| -> Result<Response, AppError> {
        let html = LoginTemplate {
            cart_count,
            user_name: None,
            error: Some(msg.to_string()),
            email: email.to_string(),
        }
        .render()?;
        Ok(Html(html).into_response())
    };

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
