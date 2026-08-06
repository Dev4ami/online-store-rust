//! Model pengguna untuk autentikasi.

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub name: String,
    #[allow(dead_code)] // dipakai mulai M6 (admin)
    pub role: String,
}

impl User {
    /// Dipakai mulai M6 (admin panel).
    #[allow(dead_code)]
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }

    /// Buat user baru. Email disimpan apa adanya; keunikan dijaga index lower(email).
    pub async fn create(
        pool: &PgPool,
        email: &str,
        password_hash: &str,
        name: &str,
    ) -> Result<User, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (email, password_hash, name)
            VALUES ($1, $2, $3)
            RETURNING id, email, password_hash, name, role
            "#,
            email,
            password_hash,
            name
        )
        .fetch_one(pool)
        .await
    }

    /// Cari user berdasarkan email (case-insensitive).
    pub async fn by_email(pool: &PgPool, email: &str) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"
            SELECT id, email, password_hash, name, role
            FROM users
            WHERE lower(email) = lower($1)
            "#,
            email
        )
        .fetch_optional(pool)
        .await
    }

    /// Ambil user berdasarkan id.
    pub async fn by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"
            SELECT id, email, password_hash, name, role
            FROM users
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await
    }
}
