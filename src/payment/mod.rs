//! Abstraksi payment gateway.
//!
//! Trait `PaymentGateway` menyembunyikan detail gateway konkret. Order & handler
//! bicara ke trait ini saja; ganti implementor (Dummy -> Midtrans/Xendit/Stripe)
//! cukup di `main.rs` tanpa mengubah logika bisnis.

pub mod dummy;

use async_trait::async_trait;
use axum::http::HeaderMap;
use rust_decimal::Decimal;
use uuid::Uuid;

/// Data yang dibutuhkan gateway untuk memulai pembayaran satu order.
/// Sebagian field belum dipakai DummyGateway tapi disiapkan untuk gateway nyata.
#[allow(dead_code)]
pub struct PaymentRequest {
    pub order_id: Uuid,
    pub number: i64,
    pub amount: Decimal,
    pub email: String,
    pub customer_name: String,
}

/// Instruksi bayar yang dikembalikan gateway.
/// `pay_url`/`method` dipakai gateway nyata (redirect eksternal); dummy render internal.
#[allow(dead_code)]
pub struct PaymentInstruction {
    /// Ke mana user diarahkan untuk membayar (URL gateway, atau halaman internal untuk dummy).
    pub pay_url: String,
    /// Referensi transaksi dari gateway.
    pub reference: String,
    /// Nama metode/gateway (mis. "dummy", "midtrans").
    pub method: String,
}

/// Status pembayaran ternormalisasi dari notifikasi gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentStatus {
    Paid,
    Pending,
    Failed,
}

/// Hasil parse + verifikasi webhook, sudah ternormalisasi lintas gateway.
pub struct WebhookResult {
    pub order_id: Uuid,
    pub reference: String,
    pub status: PaymentStatus,
}

/// Kontrak gateway pembayaran. Semua implementor harus `Send + Sync`
/// (dipegang di `AppState` via `Arc<dyn PaymentGateway>`).
#[async_trait]
pub trait PaymentGateway: Send + Sync {
    /// Nama gateway (untuk logging & disimpan sebagai `payment_method`).
    fn name(&self) -> &'static str;

    /// Mulai pembayaran: minta gateway membuat transaksi, balikin instruksi bayar.
    async fn create_payment(&self, req: &PaymentRequest) -> anyhow::Result<PaymentInstruction>;

    /// Verifikasi tanda tangan & parse body webhook mentah -> hasil ternormalisasi.
    /// Error bila signature tak valid atau payload tak bisa diparse (handler balas 400).
    fn parse_webhook(&self, headers: &HeaderMap, body: &[u8]) -> anyhow::Result<WebhookResult>;
}
