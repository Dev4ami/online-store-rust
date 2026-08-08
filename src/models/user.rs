//! Model pengguna untuk autentikasi.

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub name: String,
    pub role: String,
}

/// Hasil `ensure_admin` untuk logging startup.
#[derive(Debug, PartialEq, Eq)]
pub enum AdminSeed {
    /// Akun admin baru dibuat dari env.
    Created,
    /// Akun yang sudah ada dipromosikan ke admin.
    Promoted,
    /// Sudah admin — tak ada perubahan.
    AlreadyAdmin,
    /// Email belum terdaftar & tak ada ADMIN_PASSWORD → dilewati.
    Skipped,
}

impl User {
    /// True bila user punya peran admin.
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }

    /// Pastikan ada akun admin untuk `email` saat startup (idempoten).
    ///
    /// - `password_hash = Some(..)` (env `ADMIN_PASSWORD` diset): **upsert** —
    ///   buat akun admin baru bila email belum ada, atau promote akun yang sudah
    ///   ada. Password akun lama TIDAK ditimpa (aman bila admin sudah ganti sandi).
    ///   Cocok untuk deploy toko baru: admin langsung siap tanpa register manual.
    /// - `password_hash = None`: hanya promote akun yang sudah ada (email belum
    ///   terdaftar → dilewati; tak bisa buat akun tanpa sandi).
    ///
    /// Catatan: tak pernah men-demote admin lama. Ganti admin (turunkan yang lama)
    /// harus manual di DB.
    pub async fn ensure_admin(
        pool: &PgPool,
        email: &str,
        password_hash: Option<&str>,
    ) -> Result<AdminSeed, sqlx::Error> {
        // Sudah ada akun dgn email ini? (case-insensitive)
        if let Some(existing) = Self::by_email(pool, email).await? {
            if existing.role == "admin" {
                return Ok(AdminSeed::AlreadyAdmin);
            }
            sqlx::query!(r#"UPDATE users SET role = 'admin' WHERE id = $1"#, existing.id)
                .execute(pool)
                .await?;
            return Ok(AdminSeed::Promoted);
        }
        // Email belum terdaftar. Seed akun admin baru hanya bila sandi tersedia.
        match password_hash {
            Some(hash) => {
                sqlx::query!(
                    r#"INSERT INTO users (email, password_hash, name, role)
                       VALUES ($1, $2, 'Admin', 'admin')"#,
                    email,
                    hash
                )
                .execute(pool)
                .await?;
                Ok(AdminSeed::Created)
            }
            None => Ok(AdminSeed::Skipped),
        }
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
