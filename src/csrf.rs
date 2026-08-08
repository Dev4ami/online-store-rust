//! Proteksi CSRF: pola synchronizer-token.
//!
//! Satu token acak per-session disimpan di session store, disematkan sebagai
//! hidden field `csrf_token` di tiap form yang mengubah state, lalu divalidasi
//! (constant-time) oleh [`middleware`] pada setiap request mutating.
//!
//! Token diekspos ke template lewat task-local + [`token()`] — meniru idiom
//! `templates::store_name()` — sehingga tak perlu menambah field ke tiap struct
//! template. Middleware mengeksekusi handler di dalam [`TOKEN`]`.scope(...)`,
//! jadi `{{ crate::csrf::token() }}` selalu mengembalikan token session aktif.
//!
//! Endpoint webhook (`/webhook/payment`) dikecualikan: itu server-to-server,
//! diautentikasi oleh signature gateway, bukan cookie session.

use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::{Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use subtle::ConstantTimeEq;
use tower_sessions::Session;
use uuid::Uuid;

/// Key token di session store.
const SESSION_KEY: &str = "csrf_token";
/// Nama hidden field di form.
const FIELD: &str = "csrf_token";
/// Path yang dikecualikan dari cek CSRF (webhook server-to-server).
const EXEMPT_PATHS: &[&str] = &["/webhook/payment"];
/// Batas buffer body urlencoded saat memvalidasi (form kami jauh di bawah ini).
const MAX_BODY: usize = 64 * 1024;

tokio::task_local! {
    /// Token CSRF session aktif, di-scope oleh [`middleware`] selama handler jalan.
    static TOKEN: String;
}

/// Token CSRF untuk request saat ini; `""` bila dipanggil di luar scope middleware.
/// Dipanggil langsung dari template: `{{ crate::csrf::token() }}`.
pub fn token() -> String {
    TOKEN.try_with(|t| t.clone()).unwrap_or_default()
}

/// Token acak baru (~244 bit): dua UUID v4 digabung.
fn new_token() -> String {
    let mut s = Uuid::new_v4().simple().to_string();
    s.push_str(&Uuid::new_v4().simple().to_string());
    s
}

/// Ambil token session, atau buat + simpan bila belum ada.
async fn get_or_create(session: &Session) -> String {
    if let Ok(Some(tok)) = session.get::<String>(SESSION_KEY).await {
        return tok;
    }
    let tok = new_token();
    // Kegagalan simpan tak fatal: token tetap dipakai request ini; percobaan
    // berikutnya membuat ulang. Cek CSRF tetap aman (mismatch → ditolak).
    let _ = session.insert(SESSION_KEY, tok.clone()).await;
    tok
}

/// Bandingkan token secara constant-time. `false` bila `expected` kosong
/// (session belum punya token → tak ada yang bisa dicocokkan).
fn tokens_match(expected: &str, submitted: &str) -> bool {
    if expected.is_empty() {
        return false;
    }
    expected.as_bytes().ct_eq(submitted.as_bytes()).into()
}

/// Ekstrak satu field dari body `application/x-www-form-urlencoded`.
fn extract_urlencoded_field(body: &[u8], key: &str) -> Option<String> {
    form_urlencoded::parse(body)
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

/// Validasi token yang disubmit terhadap token session. Dipakai handler
/// multipart (yang mem-parse body sendiri, jadi dilewati middleware).
pub async fn verify(session: &Session, submitted: &str) -> bool {
    let expected = get_or_create(session).await;
    tokens_match(&expected, submitted)
}

fn is_mutating(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn content_type<'a>(req: &'a Request) -> &'a str {
    req.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, "CSRF token tidak valid").into_response()
}

/// Middleware CSRF. Wajib berjalan di dalam session layer (agar `Session`
/// tersedia). Untuk request mutating non-exempt dengan body urlencoded:
/// buffer body, cocokkan `csrf_token`, tolak 403 bila gagal. Body multipart
/// dilewati (divalidasi di handler). Selalu men-scope [`TOKEN`] agar template
/// bisa menyematkan token pada respons.
pub async fn middleware(session: Session, req: Request, next: Next) -> Response {
    let expected = get_or_create(&session).await;

    let path = req.uri().path().to_string();
    let exempt = EXEMPT_PATHS.contains(&path.as_str());

    if is_mutating(req.method()) && !exempt {
        let ct = content_type(&req);
        if ct.starts_with("application/x-www-form-urlencoded") {
            // Buffer body untuk baca token, lalu rebuild request agar handler
            // tetap menerima body utuh.
            let (parts, body) = req.into_parts();
            let bytes = match axum::body::to_bytes(body, MAX_BODY).await {
                Ok(b) => b,
                Err(_) => return forbidden(),
            };
            let submitted = extract_urlencoded_field(&bytes, FIELD).unwrap_or_default();
            if !tokens_match(&expected, &submitted) {
                return forbidden();
            }
            let req = Request::from_parts(parts, Body::from(Bytes::from(bytes)));
            return TOKEN.scope(expected, next.run(req)).await;
        } else if ct.starts_with("multipart/form-data") {
            // Handler mem-parse multipart & memanggil `verify` sendiri.
            return TOKEN.scope(expected, next.run(req)).await;
        } else {
            // Mutating tanpa body form yang bisa divalidasi → tolak.
            return forbidden();
        }
    }

    TOKEN.scope(expected, next.run(req)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::routing::{get, post};
    use tower::ServiceExt; // oneshot
    use tower_sessions::{MemoryStore, SessionManagerLayer};

    #[test]
    fn tokens_match_basic() {
        assert!(tokens_match("abc123", "abc123"));
        assert!(!tokens_match("abc123", "abc124"));
        assert!(!tokens_match("abc123", ""));
        // Expected kosong tak pernah cocok, bahkan dengan submit kosong.
        assert!(!tokens_match("", ""));
        assert!(!tokens_match("", "x"));
    }

    #[test]
    fn extract_field_works() {
        let body = b"product_id=42&csrf_token=deadbeef&qty=1";
        assert_eq!(
            extract_urlencoded_field(body, "csrf_token").as_deref(),
            Some("deadbeef")
        );
        assert_eq!(extract_urlencoded_field(body, "missing"), None);
        // URL-encoded value ter-decode.
        let enc = b"csrf_token=a%2Bb%3Dc";
        assert_eq!(
            extract_urlencoded_field(enc, "csrf_token").as_deref(),
            Some("a+b=c")
        );
    }

    /// Router minimal: GET `/form` menaruh token di scope, POST `/act` & webhook.
    fn app() -> Router {
        let store = MemoryStore::default();
        let layer = SessionManagerLayer::new(store).with_secure(false);
        Router::new()
            .route("/form", get(|| async { token() }))
            .route("/act", post(|| async { "ok" }))
            .route("/webhook/payment", post(|| async { "ok" }))
            .layer(axum::middleware::from_fn(middleware))
            .layer(layer)
    }

    fn set_cookie(res: &Response) -> String {
        res.headers()
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or_default().to_string())
            .unwrap_or_default()
    }

    async fn body_string(res: Response) -> String {
        let bytes = axum::body::to_bytes(res.into_body(), MAX_BODY).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    /// GET membuat session + mengembalikan token; POST diuji dengan/ tanpa token.
    #[tokio::test]
    async fn full_flow() {
        let app = app();

        // (a) GET → dapat cookie session + token di body.
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/form")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let cookie = set_cookie(&res);
        assert!(!cookie.is_empty(), "harus set cookie session");
        let tok = body_string(res).await;
        assert!(!tok.is_empty(), "token tak boleh kosong");

        let form = |t: &str| format!("csrf_token={t}");

        // (b) POST dengan cookie tapi tanpa token → 403.
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/act")
                    .header(header::COOKIE, &cookie)
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from("foo=bar"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        // (c) POST dengan token benar → 200.
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/act")
                    .header(header::COOKIE, &cookie)
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(form(&tok)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // (d) POST dengan token salah → 403.
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/act")
                    .header(header::COOKIE, &cookie)
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(form("salah")))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        // (e) POST webhook tanpa token → lolos (exempt).
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/webhook/payment")
                    .header(header::COOKIE, &cookie)
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from("status=paid"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
