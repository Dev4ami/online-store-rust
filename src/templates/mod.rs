//! Struct template Askama (di-render compile-time).

use askama::Template;
use rust_decimal::Decimal;

use crate::cart::CartLine;
use crate::models::product::Product;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub products: Vec<Product>,
    pub cart_count: i32,
}

#[derive(Template)]
#[template(path = "product.html")]
pub struct ProductTemplate {
    pub product: Product,
    pub cart_count: i32,
}

/// Halaman keranjang penuh.
#[derive(Template)]
#[template(path = "cart.html")]
pub struct CartTemplate {
    pub lines: Vec<CartLine>,
    pub grand_total: String,
    pub cart_count: i32,
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
