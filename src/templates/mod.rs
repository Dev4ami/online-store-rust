//! Struct template Askama (di-render compile-time).

use askama::Template;

use crate::models::product::Product;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub products: Vec<Product>,
}

#[derive(Template)]
#[template(path = "product.html")]
pub struct ProductTemplate {
    pub product: Product,
}
