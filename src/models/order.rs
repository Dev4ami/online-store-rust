//! Model pesanan + pembuatan order transaksional dari keranjang.

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::product::{Product, format_rupiah};

#[derive(Debug, Clone)]
pub struct Order {
    pub id: Uuid,
    pub number: i64,
    pub user_id: Option<Uuid>,
    pub email: String,
    pub customer_name: String,
    pub phone: String,
    pub shipping_address: String,
    pub status: String,
    #[allow(dead_code)] // ditampilkan terpisah saat ongkir/pajak masuk
    pub subtotal: Decimal,
    pub total: Decimal,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct OrderItem {
    pub product_id: Option<Uuid>,
    pub product_name: String,
    pub unit_price: Decimal,
    pub qty: i32,
    pub line_total: Decimal,
}

impl Order {
    pub fn total_display(&self) -> String {
        format_rupiah(self.total)
    }
    #[allow(dead_code)] // dipakai saat ongkir/pajak ditampilkan terpisah
    pub fn subtotal_display(&self) -> String {
        format_rupiah(self.subtotal)
    }
    /// Label status ramah pembaca (Indonesia).
    pub fn status_label(&self) -> &'static str {
        match self.status.as_str() {
            "pending" => "Menunggu Pembayaran",
            "paid" => "Sudah Dibayar",
            "cancelled" => "Dibatalkan",
            _ => "—",
        }
    }
}

impl OrderItem {
    pub fn unit_price_display(&self) -> String {
        format_rupiah(self.unit_price)
    }
    pub fn line_total_display(&self) -> String {
        format_rupiah(self.line_total)
    }
}

/// Data yang diisi pelanggan saat checkout.
pub struct CheckoutInfo {
    pub email: String,
    pub customer_name: String,
    pub phone: String,
    pub shipping_address: String,
}

/// Kesalahan spesifik proses checkout.
#[derive(Debug)]
pub enum CheckoutError {
    EmptyCart,
    /// Stok tak cukup untuk produk tertentu (nama produk).
    OutOfStock(String),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for CheckoutError {
    fn from(e: sqlx::Error) -> Self {
        CheckoutError::Db(e)
    }
}

impl Order {
    /// Buat order dari isi cart secara transaksional.
    /// - Snapshot nama & harga produk saat ini.
    /// - Kurangi stok dengan guard `stock >= qty` (cegah oversell).
    /// - Bila ada satu item gagal, seluruh transaksi di-rollback.
    ///
    /// `items` = daftar (product_id, qty) dari cart.
    pub async fn create_from_cart(
        pool: &PgPool,
        user_id: Option<Uuid>,
        info: &CheckoutInfo,
        items: &[(Uuid, i32)],
    ) -> Result<Order, CheckoutError> {
        if items.is_empty() {
            return Err(CheckoutError::EmptyCart);
        }

        let mut tx = pool.begin().await?;

        // Ambil produk terkini (untuk snapshot harga/nama). Lock baris agar konsisten.
        let ids: Vec<Uuid> = items.iter().map(|(id, _)| *id).collect();
        let products = Product::by_ids(&mut *tx, &ids).await?;
        let by_id: std::collections::HashMap<Uuid, &Product> =
            products.iter().map(|p| (p.id, p)).collect();

        // Hitung subtotal & siapkan item.
        let mut subtotal = Decimal::ZERO;
        let mut prepared: Vec<OrderItem> = Vec::new();
        for (pid, qty) in items {
            let product = by_id
                .get(pid)
                .ok_or_else(|| CheckoutError::OutOfStock("produk tidak tersedia".to_string()))?;
            if *qty <= 0 {
                continue;
            }
            let line_total = product.price * Decimal::from(*qty);
            subtotal += line_total;
            prepared.push(OrderItem {
                product_id: Some(product.id),
                product_name: product.name.clone(),
                unit_price: product.price,
                qty: *qty,
                line_total,
            });
        }
        if prepared.is_empty() {
            return Err(CheckoutError::EmptyCart);
        }

        // Ongkir belum ada -> total = subtotal (siap dikembangkan).
        let total = subtotal;

        // Insert header order.
        let order = sqlx::query_as!(
            Order,
            r#"
            INSERT INTO orders
                (user_id, email, customer_name, phone, shipping_address, subtotal, total)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, number, user_id, email, customer_name, phone,
                      shipping_address, status, subtotal, total,
                      created_at as "created_at: chrono::DateTime<chrono::Utc>"
            "#,
            user_id,
            info.email,
            info.customer_name,
            info.phone,
            info.shipping_address,
            subtotal,
            total,
        )
        .fetch_one(&mut *tx)
        .await?;

        // Insert item + kurangi stok dengan guard anti-oversell.
        for item in &prepared {
            let pid = item.product_id.expect("product_id ada saat pembuatan");

            let rows = sqlx::query!(
                r#"UPDATE products SET stock = stock - $1 WHERE id = $2 AND stock >= $1"#,
                item.qty,
                pid,
            )
            .execute(&mut *tx)
            .await?
            .rows_affected();

            if rows == 0 {
                // Stok tak cukup -> batalkan seluruh order.
                tx.rollback().await?;
                return Err(CheckoutError::OutOfStock(item.product_name.clone()));
            }

            sqlx::query!(
                r#"
                INSERT INTO order_items
                    (order_id, product_id, product_name, unit_price, qty, line_total)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
                order.id,
                item.product_id,
                item.product_name,
                item.unit_price,
                item.qty,
                item.line_total,
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(order)
    }

    /// Ambil order + itemnya. Batasi akses lewat parameter (owner check di handler).
    pub async fn by_id(pool: &PgPool, id: Uuid) -> Result<Option<Order>, sqlx::Error> {
        sqlx::query_as!(
            Order,
            r#"
            SELECT id, number, user_id, email, customer_name, phone,
                   shipping_address, status, subtotal, total,
                   created_at as "created_at: chrono::DateTime<chrono::Utc>"
            FROM orders WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    /// Ambil item-item sebuah order.
    pub async fn items(pool: &PgPool, order_id: Uuid) -> Result<Vec<OrderItem>, sqlx::Error> {
        sqlx::query_as!(
            OrderItem,
            r#"
            SELECT product_id, product_name, unit_price, qty, line_total
            FROM order_items WHERE order_id = $1
            ORDER BY product_name
            "#,
            order_id
        )
        .fetch_all(pool)
        .await
    }

    /// Riwayat order milik seorang user, terbaru dulu.
    pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Order>, sqlx::Error> {
        sqlx::query_as!(
            Order,
            r#"
            SELECT id, number, user_id, email, customer_name, phone,
                   shipping_address, status, subtotal, total,
                   created_at as "created_at: chrono::DateTime<chrono::Utc>"
            FROM orders WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
            user_id
        )
        .fetch_all(pool)
        .await
    }
}
