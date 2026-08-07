//! Model produk + query katalog.

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

/// Urutan hasil katalog. Dipetakan dari/ke param query `sort`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductSort {
    Newest,
    PriceAsc,
    PriceDesc,
    Name,
}

impl ProductSort {
    /// Parse dari nilai query string; nilai tak dikenal → `Newest`.
    pub fn from_query(s: &str) -> Self {
        match s {
            "price_asc" => Self::PriceAsc,
            "price_desc" => Self::PriceDesc,
            "name" => Self::Name,
            _ => Self::Newest,
        }
    }

    /// Nilai kanonik untuk di-echo balik ke `<option value=...>`/`selected`.
    pub fn as_query(&self) -> &'static str {
        match self {
            Self::Newest => "newest",
            Self::PriceAsc => "price_asc",
            Self::PriceDesc => "price_desc",
            Self::Name => "name",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Product {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub price: Decimal,
    pub stock: i32,
    pub image_url: Option<String>,
    pub is_active: bool,
}

/// Data produk tervalidasi dari form admin (untuk create/update).
pub struct NewProduct {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub price: Decimal,
    pub stock: i32,
    pub image_url: Option<String>,
    pub is_active: bool,
}

/// Jumlah produk per halaman katalog.
pub const PER_PAGE: i64 = 24;

/// Satu halaman hasil katalog: produk halaman ini + total (untuk hitung jumlah halaman).
pub struct ProductPage {
    pub items: Vec<Product>,
    pub total: i64,
}

impl Product {
    /// Harga terformat Rupiah, mis. "Rp1.250.000".
    pub fn price_display(&self) -> String {
        format_rupiah(self.price)
    }

    /// Cari, filter, urutkan, dan paginasi produk aktif untuk katalog publik.
    /// - `q`: cocok pada nama (case-insensitive); metachar LIKE di-escape.
    /// - `min`/`max`: batas harga inklusif.
    /// - `sort`: urutan hasil (di SQL, agar konsisten lintas halaman).
    /// - `limit`/`offset`: jendela halaman.
    ///
    /// Dua query dengan `WHERE` identik: `COUNT(*)` untuk total (hitung jumlah
    /// halaman) + `SELECT ... LIMIT/OFFSET` untuk isi halaman. Pengurutan
    /// dilakukan di SQL via `ORDER BY CASE` — WAJIB sebelum `LIMIT` agar tak
    /// hanya mengurutkan potongan halaman.
    pub async fn search(
        pool: &PgPool,
        q: Option<&str>,
        min: Option<Decimal>,
        max: Option<Decimal>,
        sort: ProductSort,
        limit: i64,
        offset: i64,
    ) -> Result<ProductPage, sqlx::Error> {
        // Escape metachar LIKE agar user tak bisa menyisipkan wildcard.
        let pattern = q.map(|s| format!("%{}%", escape_like(s)));
        let sort_key = sort.as_query();

        // Total baris yang cocok (untuk pagination). WHERE sama persis dengan query halaman.
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) FROM products
            WHERE is_active = TRUE
              AND ($1::text IS NULL OR name ILIKE $1 ESCAPE '\')
              AND ($2::numeric IS NULL OR price >= $2)
              AND ($3::numeric IS NULL OR price <= $3)
            "#,
            pattern,
            min,
            max,
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

        let items = sqlx::query_as!(
            Product,
            r#"
            SELECT id, slug, name, description, price, stock, image_url, is_active
            FROM products
            WHERE is_active = TRUE
              AND ($1::text IS NULL OR name ILIKE $1 ESCAPE '\')
              AND ($2::numeric IS NULL OR price >= $2)
              AND ($3::numeric IS NULL OR price <= $3)
            ORDER BY
              CASE WHEN $4 = 'price_asc'  THEN price END ASC,
              CASE WHEN $4 = 'price_desc' THEN price END DESC,
              CASE WHEN $4 = 'name'       THEN lower(name) END ASC,
              created_at DESC
            LIMIT $5 OFFSET $6
            "#,
            pattern,
            min,
            max,
            sort_key,
            limit,
            offset,
        )
        .fetch_all(pool)
        .await?;

        Ok(ProductPage { items, total })
    }

    /// Ambil satu produk aktif berdasarkan slug.
    pub async fn by_slug(pool: &PgPool, slug: &str) -> Result<Option<Product>, sqlx::Error> {
        sqlx::query_as!(
            Product,
            r#"
            SELECT id, slug, name, description, price, stock, image_url, is_active
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
            SELECT id, slug, name, description, price, stock, image_url, is_active
            FROM products
            WHERE id = $1 AND is_active = TRUE
            "#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    /// Ambil banyak produk aktif sekaligus (untuk isi keranjang / checkout).
    /// Generik atas executor agar bisa dipakai di dalam transaksi maupun pool.
    pub async fn by_ids<'e, E>(executor: E, ids: &[Uuid]) -> Result<Vec<Product>, sqlx::Error>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query_as!(
            Product,
            r#"
            SELECT id, slug, name, description, price, stock, image_url, is_active
            FROM products
            WHERE id = ANY($1) AND is_active = TRUE
            "#,
            ids
        )
        .fetch_all(executor)
        .await
    }

    // --- Query khusus admin (M6): lihat & ubah semua produk, termasuk nonaktif. ---

    /// Semua produk (aktif + nonaktif), terbaru dulu — untuk daftar admin.
    pub async fn list_all(pool: &PgPool) -> Result<Vec<Product>, sqlx::Error> {
        sqlx::query_as!(
            Product,
            r#"
            SELECT id, slug, name, description, price, stock, image_url, is_active
            FROM products
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(pool)
        .await
    }

    /// Ambil produk berdasarkan id tanpa filter is_active (untuk form edit admin).
    pub async fn by_id_any(pool: &PgPool, id: Uuid) -> Result<Option<Product>, sqlx::Error> {
        sqlx::query_as!(
            Product,
            r#"
            SELECT id, slug, name, description, price, stock, image_url, is_active
            FROM products
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    /// Cek apakah slug sudah dipakai produk lain (kecuali `except_id`).
    /// Dipakai sebelum create/update agar bisa balas pesan ramah, bukan 500 dari UNIQUE.
    pub async fn slug_taken(
        pool: &PgPool,
        slug: &str,
        except_id: Option<Uuid>,
    ) -> Result<bool, sqlx::Error> {
        let row = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                SELECT 1 FROM products
                WHERE slug = $1 AND ($2::uuid IS NULL OR id <> $2)
            )"#,
            slug,
            except_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(row.unwrap_or(false))
    }

    /// Buat produk baru. Balikin produk tersimpan.
    pub async fn create(pool: &PgPool, p: &NewProduct) -> Result<Product, sqlx::Error> {
        sqlx::query_as!(
            Product,
            r#"
            INSERT INTO products (slug, name, description, price, stock, image_url, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, slug, name, description, price, stock, image_url, is_active
            "#,
            p.slug,
            p.name,
            p.description,
            p.price,
            p.stock,
            p.image_url,
            p.is_active,
        )
        .fetch_one(pool)
        .await
    }

    /// Perbarui produk. Balikin `true` bila baris berubah (id ada).
    pub async fn update(pool: &PgPool, id: Uuid, p: &NewProduct) -> Result<bool, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            UPDATE products
               SET slug = $2, name = $3, description = $4,
                   price = $5, stock = $6, image_url = $7, is_active = $8
             WHERE id = $1
            "#,
            id,
            p.slug,
            p.name,
            p.description,
            p.price,
            p.stock,
            p.image_url,
            p.is_active,
        )
        .execute(pool)
        .await?
        .rows_affected();
        Ok(rows > 0)
    }

    /// Hapus produk permanen. `order_items.product_id` di-set NULL (FK ON DELETE SET NULL),
    /// nama & harga snapshot di order_items tetap utuh. Balikin `true` bila terhapus.
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let rows = sqlx::query!(r#"DELETE FROM products WHERE id = $1"#, id)
            .execute(pool)
            .await?
            .rows_affected();
        Ok(rows > 0)
    }
}

/// Escape metachar pola LIKE/ILIKE (`\`, `%`, `_`) dengan backslash.
/// Dipakai agar input pencarian diperlakukan sebagai teks literal, bukan wildcard.
/// Pasangkan dengan klausa `ESCAPE '\'`.
fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
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
