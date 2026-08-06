//! Utilitas autentikasi: hashing password (argon2) + sesi login.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng};
use argon2::Argon2;
use tower_sessions::Session;
use uuid::Uuid;

const USER_ID_KEY: &str = "user_id";

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

/// Ambil nama tampilan user yang login (untuk header). Fallback ke email.
pub async fn current_user_name(
    session: &Session,
    pool: &sqlx::PgPool,
) -> Option<String> {
    let id = current_user_id(session).await?;
    let user = crate::models::user::User::by_id(pool, id).await.ok().flatten()?;
    if user.name.is_empty() {
        Some(user.email)
    } else {
        Some(user.name)
    }
}
