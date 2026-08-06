//! Tipe error aplikasi + konversi ke HTTP response.

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("halaman tidak ditemukan")]
    NotFound,

    #[error("kesalahan database: {0}")]
    Database(#[from] sqlx::Error),

    #[error("gagal render template: {0}")]
    Render(#[from] askama::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, judul, pesan) = match self {
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                "404 — Tidak Ditemukan",
                "Produk atau halaman yang kamu cari tidak ada.".to_string(),
            ),
            AppError::Database(e) => {
                tracing::error!("database error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "500 — Kesalahan Server",
                    "Terjadi kesalahan pada server. Coba lagi nanti.".to_string(),
                )
            }
            AppError::Render(e) => {
                tracing::error!("render error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "500 — Kesalahan Server",
                    "Terjadi kesalahan pada server. Coba lagi nanti.".to_string(),
                )
            }
        };

        let body = format!(
            "<!doctype html><html lang=\"id\"><head><meta charset=\"utf-8\">\
             <title>{judul}</title></head><body style=\"font-family:system-ui;text-align:center;padding:4rem\">\
             <h1>{judul}</h1><p>{pesan}</p><p><a href=\"/\">Kembali ke beranda</a></p></body></html>"
        );

        (status, Html(body)).into_response()
    }
}
