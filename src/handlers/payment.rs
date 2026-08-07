//! Handler pembayaran: halaman bayar + endpoint webhook gateway.

use askama::Template;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use tower_sessions::Session;
use uuid::Uuid;

use crate::auth;
use crate::cart::Cart;
use crate::error::AppError;
use crate::models::order::Order;
use crate::payment::{PaymentRequest, PaymentStatus};
use crate::state::AppState;
use crate::templates::PayTemplate;

/// GET /pay/{id} — halaman bayar. Minta gateway membuat transaksi lalu tampilkan instruksi.
/// Akses: pemilik (login) ATAU order tercatat di session (guest) — sama seperti detail order.
pub async fn pay_page(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let order = Order::by_id(&state.pool, id).await?.ok_or(AppError::NotFound)?;

    // Kontrol akses.
    let current = auth::current_user_id(&session).await;
    let is_owner = matches!((order.user_id, current), (Some(o), Some(c)) if o == c);
    if !is_owner && !auth::remembers_order(&session, id).await {
        return Err(AppError::NotFound);
    }

    // Sudah dibayar -> tak ada yang perlu dibayar, kembali ke detail.
    if !order.is_pending() {
        return Ok(Redirect::to(&format!("/order/{id}")).into_response());
    }

    // Minta gateway membuat transaksi (dummy: tanpa jaringan).
    let req = PaymentRequest {
        order_id: order.id,
        number: order.number,
        amount: order.total,
        email: order.email.clone(),
        customer_name: order.customer_name.clone(),
    };
    let instruction = state
        .gateway
        .create_payment(&req)
        .await
        .map_err(|e| AppError::Internal(format!("gagal membuat pembayaran: {e}")))?;

    let cart_count = Cart::load(&session).await.total_qty();
    let (user_name, is_admin) = auth::current_user_header(&session, &state.pool).await;

    let html = PayTemplate {
        cart_count,
        user_name,
        is_admin,
        order,
        reference: instruction.reference,
        // Disodorkan ke form simulasi dummy agar bisa mengirim webhook bertanda tangan.
        // Gateway nyata: user diarahkan ke `instruction.pay_url` eksternal, tanpa secret di halaman.
        dummy_secret: state.dummy_secret.clone(),
    }
    .render()?;
    Ok(Html(html).into_response())
}

/// POST /webhook/payment — notifikasi dari gateway (server-to-server, TANPA session).
/// Verifikasi via `gateway.parse_webhook`, lalu tandai order lunas (idempoten).
pub async fn webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Verifikasi signature + parse. Gagal -> 400 (jangan sentuh order).
    let result = match state.gateway.parse_webhook(&headers, &body) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("webhook ditolak: {e}");
            return (StatusCode::BAD_REQUEST, "invalid webhook").into_response();
        }
    };

    match result.status {
        PaymentStatus::Paid => {
            match Order::mark_paid(
                &state.pool,
                result.order_id,
                state.gateway.name(),
                &result.reference,
            )
            .await
            {
                Ok(true) => tracing::info!(
                    "order {} ditandai lunas (ref {})",
                    result.order_id,
                    result.reference
                ),
                Ok(false) => tracing::info!(
                    "order {} sudah lunas / tak pending — webhook diabaikan (idempoten)",
                    result.order_id
                ),
                Err(e) => {
                    tracing::error!("gagal mark_paid order {}: {e}", result.order_id);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
                }
            }
        }
        // Pending/Failed: tak ada transisi. Balas 200 agar gateway berhenti retry.
        other => tracing::info!("webhook order {} status {:?} — diabaikan", result.order_id, other),
    }

    // Selalu 200 bila terverifikasi (gateway nyata retry hanya untuk non-2xx).
    StatusCode::OK.into_response()
}
