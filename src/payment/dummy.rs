//! Gateway tiruan untuk pengembangan lokal — TANPA panggilan jaringan.
//!
//! Meniru bentuk gateway nyata (buat transaksi + verifikasi webhook bertanda tangan)
//! supaya alur `pending -> paid` bisa dites end-to-end sebelum gateway asli dipilih.
//! Ganti dengan implementor konkret saat gateway dipilih; kontrak `PaymentGateway` sama.

use async_trait::async_trait;
use axum::http::HeaderMap;
use uuid::Uuid;

use super::{PaymentGateway, PaymentInstruction, PaymentRequest, PaymentStatus, WebhookResult};

/// Header tanda tangan simulasi (analog `X-Callback-Signature` di gateway nyata).
const SIGNATURE_HEADER: &str = "x-dummy-signature";

pub struct DummyGateway {
    /// Rahasia bersama; webhook harus menyertakannya di header signature.
    secret: String,
}

impl DummyGateway {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }
}

#[async_trait]
impl PaymentGateway for DummyGateway {
    fn name(&self) -> &'static str {
        "dummy"
    }

    async fn create_payment(&self, req: &PaymentRequest) -> anyhow::Result<PaymentInstruction> {
        // Tanpa jaringan: reference deterministik, arahkan ke halaman bayar internal.
        Ok(PaymentInstruction {
            pay_url: format!("/pay/{}", req.order_id),
            reference: format!("DUMMY-{}", req.number),
            method: self.name().to_string(),
        })
    }

    fn parse_webhook(&self, headers: &HeaderMap, body: &[u8]) -> anyhow::Result<WebhookResult> {
        // 1) Verifikasi signature: header harus sama persis dengan secret.
        //    (Gateway nyata: HMAC atas body — ganti di sini saja.)
        let sig = headers
            .get(SIGNATURE_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if sig.is_empty() || sig != self.secret {
            anyhow::bail!("signature webhook tidak valid");
        }

        // 2) Parse body form-urlencoded sederhana: order_id, reference, status.
        let body = std::str::from_utf8(body)?;
        let mut order_id: Option<Uuid> = None;
        let mut reference = String::new();
        let mut status = PaymentStatus::Pending;
        for pair in body.split('&') {
            let Some((k, v)) = pair.split_once('=') else {
                continue;
            };
            match k {
                "order_id" => order_id = Uuid::parse_str(v).ok(),
                "reference" => reference = v.to_string(),
                "status" => {
                    status = match v {
                        "paid" => PaymentStatus::Paid,
                        "failed" => PaymentStatus::Failed,
                        _ => PaymentStatus::Pending,
                    }
                }
                _ => {}
            }
        }

        let order_id = order_id.ok_or_else(|| anyhow::anyhow!("order_id tidak ada/invalid"))?;
        Ok(WebhookResult {
            order_id,
            reference,
            status,
        })
    }
}
