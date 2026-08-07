//! Struct template Askama (di-render compile-time).

use std::sync::OnceLock;

use askama::Template;
use rust_decimal::Decimal;

use crate::cart::CartLine;
use crate::models::order::{Order, OrderItem};
use crate::models::product::Product;

/// Nama toko dari env, di-set sekali saat startup. Dipanggil langsung dari
/// template (`{{ store_name() }}`) untuk brand, footer, dan judul tab —
/// tanpa perlu field di tiap struct template.
static STORE_NAME: OnceLock<String> = OnceLock::new();

/// Set nama toko (dipanggil di `main` setelah baca config). Idempoten.
pub fn set_store_name(name: String) {
    let _ = STORE_NAME.set(name);
}

/// Nama toko aktif; fallback "Toko Online" bila belum di-set.
pub fn store_name() -> &'static str {
    STORE_NAME.get().map(String::as_str).unwrap_or("Toko Online")
}

/// Satu tombol di nav pagination. `num == 0` menandai elipsis "…".
/// `current` = halaman yang sedang dibuka (precompute agar template tak perlu
/// membandingkan `&i64`, yang tak didukung Askama).
pub struct PageBtn {
    pub num: i64,
    pub current: bool,
}

/// Konteks katalog (filter + paginasi) yang dibagikan halaman penuh & partial grid.
/// `q/min/max/sort` dipakai untuk isi ulang toolbar dan membangun href pagination.
pub struct Pagination {
    pub q: String,
    pub min: String,
    pub max: String,
    pub sort: String,
    pub page: i64,
    pub total_pages: i64,
    pub pages: Vec<PageBtn>,
    pub has_prev: bool,
    pub has_next: bool,
}

/// Bangun jendela tombol halaman: selalu tampilkan 1, terakhir, dan current±2;
/// sisipkan elipsis (`num=0`) saat ada lompatan. Contoh (current=6, total=20):
/// [1, …, 4, 5, 6, 7, 8, …, 20].
pub fn page_window(current: i64, total: i64) -> Vec<PageBtn> {
    let mut out = Vec::new();
    let mut prev = 0i64;
    for n in 1..=total {
        let keep = n == 1 || n == total || (n - current).abs() <= 2;
        if keep {
            if prev != 0 && n - prev > 1 {
                out.push(PageBtn { num: 0, current: false }); // lompatan → elipsis
            }
            out.push(PageBtn { num: n, current: n == current });
            prev = n;
        }
    }
    out
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub products: Vec<Product>,
    pub cart_count: i32,
    pub user_name: Option<String>,
    pub is_admin: bool,
    pub pg: Pagination,
}

/// Partial grid produk + nav pagination (untuk swap HTMX saat filter/pindah halaman).
#[derive(Template)]
#[template(path = "catalog_grid.html")]
pub struct CatalogGridTemplate {
    pub products: Vec<Product>,
    pub pg: Pagination,
}

#[derive(Template)]
#[template(path = "product.html")]
pub struct ProductTemplate {
    pub product: Product,
    pub cart_count: i32,
    pub user_name: Option<String>,
    pub is_admin: bool,
}

/// Halaman keranjang penuh.
#[derive(Template)]
#[template(path = "cart.html")]
pub struct CartTemplate {
    pub lines: Vec<CartLine>,
    pub grand_total: String,
    pub cart_count: i32,
    pub user_name: Option<String>,
    pub is_admin: bool,
}

/// Halaman registrasi.
#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    pub cart_count: i32,
    pub user_name: Option<String>,
    pub is_admin: bool,
    pub error: Option<String>,
    pub email: String,
    pub name: String,
}

/// Halaman login.
#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub cart_count: i32,
    pub user_name: Option<String>,
    pub is_admin: bool,
    pub error: Option<String>,
    pub email: String,
}

/// Halaman checkout (form pengiriman + ringkasan).
#[derive(Template)]
#[template(path = "checkout.html")]
pub struct CheckoutTemplate {
    pub cart_count: i32,
    pub user_name: Option<String>,
    pub is_admin: bool,
    pub lines: Vec<CartLine>,
    pub grand_total: String,
    pub error: Option<String>,
    pub email: String,
    pub customer_name: String,
    pub phone: String,
    pub shipping_address: String,
}

/// Halaman konfirmasi/detail satu pesanan.
#[derive(Template)]
#[template(path = "order_detail.html")]
pub struct OrderDetailTemplate {
    pub cart_count: i32,
    pub user_name: Option<String>,
    pub is_admin: bool,
    pub order: Order,
    pub items: Vec<OrderItem>,
}

/// Riwayat pesanan.
#[derive(Template)]
#[template(path = "orders.html")]
pub struct OrdersTemplate {
    pub cart_count: i32,
    pub user_name: Option<String>,
    pub is_admin: bool,
    pub orders: Vec<Order>,
}

/// Halaman bayar satu pesanan (mode dummy: form simulasi webhook).
#[derive(Template)]
#[template(path = "pay.html")]
pub struct PayTemplate {
    pub cart_count: i32,
    pub user_name: Option<String>,
    pub is_admin: bool,
    pub order: Order,
    pub reference: String,
    pub dummy_secret: String,
}

// --- Admin (M6) ---

/// Daftar produk untuk admin (aktif + nonaktif).
#[derive(Template)]
#[template(path = "admin_products.html")]
pub struct AdminProductsTemplate {
    pub admin_name: String,
    pub products: Vec<Product>,
}

/// Form tambah/ubah produk. `editing` menentukan judul; `action_url` tujuan POST.
#[derive(Template)]
#[template(path = "admin_product_form.html")]
pub struct AdminProductFormTemplate {
    pub admin_name: String,
    pub error: Option<String>,
    pub editing: bool,
    pub action_url: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub price: String,
    pub stock: String,
    // Gambar produk saat ini (untuk preview saat edit); None saat tambah baru.
    pub current_image: Option<String>,
    pub is_active: bool,
}

/// Daftar semua pesanan untuk admin.
#[derive(Template)]
#[template(path = "admin_orders.html")]
pub struct AdminOrdersTemplate {
    pub admin_name: String,
    pub orders: Vec<Order>,
}

/// Detail satu pesanan untuk admin + aksi ubah status.
#[derive(Template)]
#[template(path = "admin_order_detail.html")]
pub struct AdminOrderDetailTemplate {
    pub admin_name: String,
    pub order: Order,
    pub items: Vec<OrderItem>,
}

/// Partial isi keranjang (untuk swap HTMX setelah update/remove).
#[derive(Template)]
#[template(path = "cart_contents.html")]
pub struct CartContentsTemplate {
    pub lines: Vec<CartLine>,
    pub grand_total: String,
    pub cart_count: i32,
}

impl CartContentsTemplate {
    pub fn new(lines: Vec<CartLine>, total: Decimal, cart_count: i32) -> Self {
        Self {
            lines,
            grand_total: crate::cart::Cart::grand_total_display(total),
            cart_count,
        }
    }
}
