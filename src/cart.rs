//! Keranjang belanja berbasis session (guest cart).
//! Disimpan di session sebagai peta product_id -> qty.

use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use crate::models::product::{Product, format_rupiah};

const CART_KEY: &str = "cart";

/// Satu baris keranjang: produk + qty + subtotal (untuk ditampilkan).
pub struct CartLine {
    pub product: Product,
    pub qty: i32,
    pub line_total: Decimal,
}

impl CartLine {
    pub fn line_total_display(&self) -> String {
        format_rupiah(self.line_total)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Cart {
    pub items: HashMap<Uuid, i32>,
}

impl Cart {
    /// Muat cart dari session (kosong bila belum ada).
    pub async fn load(session: &Session) -> Cart {
        session.get(CART_KEY).await.ok().flatten().unwrap_or_default()
    }

    /// Simpan cart ke session.
    pub async fn save(&self, session: &Session) -> Result<(), tower_sessions::session::Error> {
        session.insert(CART_KEY, self).await
    }

    /// Tambah qty untuk sebuah produk (minimal 1).
    pub fn add(&mut self, id: Uuid, qty: i32) {
        let entry = self.items.entry(id).or_insert(0);
        *entry = (*entry + qty.max(1)).max(1);
    }

    /// Set qty absolut; qty <= 0 menghapus item.
    pub fn set(&mut self, id: Uuid, qty: i32) {
        if qty <= 0 {
            self.items.remove(&id);
        } else {
            self.items.insert(id, qty);
        }
    }

    /// Hapus produk dari cart.
    pub fn remove(&mut self, id: Uuid) {
        self.items.remove(&id);
    }

    /// Total jumlah barang (untuk badge).
    pub fn total_qty(&self) -> i32 {
        self.items.values().sum()
    }

    /// Resolusi cart jadi baris-baris berisi produk + subtotal, plus grand total.
    /// Produk yang sudah tidak aktif/terhapus otomatis dilewati (di-prune dari cart).
    pub async fn detailed(
        &mut self,
        pool: &sqlx::PgPool,
    ) -> Result<(Vec<CartLine>, Decimal), sqlx::Error> {
        if self.items.is_empty() {
            return Ok((Vec::new(), Decimal::ZERO));
        }

        let ids: Vec<Uuid> = self.items.keys().copied().collect();
        let products = Product::by_ids(pool, &ids).await?;

        let mut lines = Vec::new();
        let mut grand_total = Decimal::ZERO;
        let mut valid_ids = std::collections::HashSet::new();

        for product in products {
            let qty = *self.items.get(&product.id).unwrap_or(&0);
            if qty <= 0 {
                continue;
            }
            valid_ids.insert(product.id);
            let line_total = product.price * Decimal::from(qty);
            grand_total += line_total;
            lines.push(CartLine { product, qty, line_total });
        }

        // Prune item yang produknya sudah hilang agar cart tetap konsisten.
        self.items.retain(|id, _| valid_ids.contains(id));

        // Urutkan stabil berdasar nama produk.
        lines.sort_by(|a, b| a.product.name.cmp(&b.product.name));

        Ok((lines, grand_total))
    }

    /// Grand total terformat Rupiah.
    pub fn grand_total_display(total: Decimal) -> String {
        format_rupiah(total)
    }
}
