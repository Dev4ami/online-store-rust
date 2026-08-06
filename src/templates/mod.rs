//! Struct template Askama (di-render compile-time).

use askama::Template;
use rust_decimal::Decimal;

use crate::cart::CartLine;
use crate::models::order::{Order, OrderItem};
use crate::models::product::Product;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub products: Vec<Product>,
    pub cart_count: i32,
    pub user_name: Option<String>,
}

#[derive(Template)]
#[template(path = "product.html")]
pub struct ProductTemplate {
    pub product: Product,
    pub cart_count: i32,
    pub user_name: Option<String>,
}

/// Halaman keranjang penuh.
#[derive(Template)]
#[template(path = "cart.html")]
pub struct CartTemplate {
    pub lines: Vec<CartLine>,
    pub grand_total: String,
    pub cart_count: i32,
    pub user_name: Option<String>,
}

/// Halaman registrasi.
#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    pub cart_count: i32,
    pub user_name: Option<String>,
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
    pub error: Option<String>,
    pub email: String,
}

/// Halaman checkout (form pengiriman + ringkasan).
#[derive(Template)]
#[template(path = "checkout.html")]
pub struct CheckoutTemplate {
    pub cart_count: i32,
    pub user_name: Option<String>,
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
    pub order: Order,
    pub items: Vec<OrderItem>,
}

/// Riwayat pesanan.
#[derive(Template)]
#[template(path = "orders.html")]
pub struct OrdersTemplate {
    pub cart_count: i32,
    pub user_name: Option<String>,
    pub orders: Vec<Order>,
}

/// Halaman bayar satu pesanan (mode dummy: form simulasi webhook).
#[derive(Template)]
#[template(path = "pay.html")]
pub struct PayTemplate {
    pub cart_count: i32,
    pub user_name: Option<String>,
    pub order: Order,
    pub reference: String,
    pub dummy_secret: String,
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
