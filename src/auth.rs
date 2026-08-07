//! Utilitas autentikasi: hashing password (argon2) + sesi login.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng};
use argon2::Argon2;
use tower_sessions::Session;
use uuid::Uuid;

const USER_ID_KEY: &str = "user_id";
const RECENT_ORDERS_KEY: &str = "orders_recent";

/// Hash password plaintext dengan Argon2id + salt acak.
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

/// Verifikasi password terhadap hash tersimpan.
pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    match PasswordHash::new(stored_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// Tandai user sebagai login di session.
pub async fn login(session: &Session, user_id: Uuid) -> Result<(), tower_sessions::session::Error> {
    // Cegah session fixation: putar id session saat login.
    session.cycle_id().await?;
    session.insert(USER_ID_KEY, user_id).await
}

/// Hapus status login dari session.
pub async fn logout(session: &Session) -> Result<(), tower_sessions::session::Error> {
    session.remove::<Uuid>(USER_ID_KEY).await?;
    Ok(())
}

/// Ambil user_id yang sedang login (bila ada).
pub async fn current_user_id(session: &Session) -> Option<Uuid> {
    session.get(USER_ID_KEY).await.ok().flatten()
}

/// Ingat order id di session agar guest bisa lihat konfirmasi & halaman bayar miliknya.
pub async fn remember_order(session: &Session, id: Uuid) {
    let mut ids: Vec<Uuid> = session
        .get(RECENT_ORDERS_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    if !ids.contains(&id) {
        ids.push(id);
    }
    let _ = session.insert(RECENT_ORDERS_KEY, ids).await;
}

/// Apakah session mengingat order id ini (untuk kontrol akses guest).
pub async fn remembers_order(session: &Session, id: Uuid) -> bool {
    let ids: Vec<Uuid> = session
        .get(RECENT_ORDERS_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    ids.contains(&id)
}

/// Ambil user admin yang sedang login, atau `None` (bukan login / bukan admin).
/// Handler admin memakai ini lalu balas 404 bila `None` (sembunyikan keberadaan panel).
pub async fn current_admin(
    session: &Session,
    pool: &sqlx::PgPool,
) -> Option<crate::models::user::User> {
    let id = current_user_id(session).await?;
    let user = crate::models::user::User::by_id(pool, id).await.ok().flatten()?;
    user.is_admin().then_some(user)
}

/// Info user untuk header tiap halaman: nama tampilan (fallback email) + apakah admin.
/// Satu query DB; `(None, false)` bila belum login. Dipakai semua handler yang
/// merender `base.html` agar link "Admin" bisa muncul kondisional.
pub async fn current_user_header(
    session: &Session,
    pool: &sqlx::PgPool,
) -> (Option<String>, bool) {
    let Some(id) = current_user_id(session).await else {
        return (None, false);
    };
    let Some(user) = crate::models::user::User::by_id(pool, id).await.ok().flatten() else {
        return (None, false);
    };
    let name = if user.name.is_empty() {
        user.email.clone()
    } else {
        user.name.clone()
    };
    (Some(name), user.is_admin())
}
