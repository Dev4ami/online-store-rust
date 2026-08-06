//! Model produk + query katalog.

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Product {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub price: Decimal,
    pub stock: i32,
    pub image_url: Option<String>,
}

impl Product {
    /// Harga terformat Rupiah, mis. "Rp1.250.000".
    pub fn price_display(&self) -> String {
        format_rupiah(self.price)
    }

    /// Ambil semua produk aktif, terbaru dulu.
    pub async fn list_active(pool: &PgPool) -> Result<Vec<Product>, sqlx::Error> {
        sqlx::query_as!(
            Product,
            r#"
            SELECT id, slug, name, description, price, stock, image_url
            FROM products
            WHERE is_active = TRUE
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(pool)
        .await
    }

    /// Ambil satu produk aktif berdasarkan slug.
    pub async fn by_slug(pool: &PgPool, slug: &str) -> Result<Option<Product>, sqlx::Error> {
        sqlx::query_as!(
            Product,
            r#"
            SELECT id, slug, name, description, price, stock, image_url
            FROM products
            WHERE slug = $1 AND is_active = TRUE
            "#,
            slug
        )
        .fetch_optional(pool)
        .await
    }

    /// Ambil satu produk aktif berdasarkan id.
    pub async fn by_id(pool: &PgPool, id: Uuid) -> Result<Option<Product>, sqlx::Error> {
        sqlx::query_as!(
            Product,
            r#"
            SELECT id, slug, name, description, price, stock, image_url
            FROM products
            WHERE id = $1 AND is_active = TRUE
            "#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    /// Ambil banyak produk aktif sekaligus (untuk isi keranjang).
    pub async fn by_ids(pool: &PgPool, ids: &[Uuid]) -> Result<Vec<Product>, sqlx::Error> {
        sqlx::query_as!(
            Product,
            r#"
            SELECT id, slug, name, description, price, stock, image_url
            FROM products
            WHERE id = ANY($1) AND is_active = TRUE
            "#,
            ids
        )
        .fetch_all(pool)
        .await
    }
}

/// Format Decimal jadi Rupiah dengan pemisah ribuan titik.
/// Contoh: 1250000.00 -> "Rp1.250.000". Pecahan sen dibuang (harga rupiah bulat).
pub fn format_rupiah(value: Decimal) -> String {
    let bulat = value.trunc().abs();
    let digits = bulat.to_string(); // tanpa desimal karena sudah trunc
    let mut hasil = String::new();
    let bytes = digits.as_bytes();
    let len = bytes.len();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            hasil.push('.');
        }
        hasil.push(*b as char);
    }
    let tanda = if value.is_sign_negative() { "-" } else { "" };
    format!("{tanda}Rp{hasil}")
}
