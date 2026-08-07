# Online Store (Rust)

Toko online sederhana namun serius, dibangun dengan **Rust** — fokus hemat resource server, render server-side, dan siap disambung payment gateway nyata.

## Stack

- **Web**: [Axum](https://github.com/tokio-rs/axum) 0.8
- **Template**: [Askama](https://github.com/rinja-rs/askama) 0.16 (render server-side, compile-time)
- **Interaktivitas**: [HTMX](https://htmx.org/) (self-host, tanpa build step JS)
- **Database**: PostgreSQL + [SQLx](https://github.com/launchbadge/sqlx) 0.8 (query cek-compile)
- **Auth**: Argon2id + session (`tower-sessions`, disimpan di Postgres)
- **Uang**: `NUMERIC(12,2)` ↔ `rust_decimal::Decimal`
- **Edition**: Rust 2024

## Fitur

- **Katalog**: daftar produk, halaman detail, pencarian nama, filter harga, urutan (terbaru/harga/nama), pagination — semua live via HTMX
- **Keranjang**: berbasis session, badge jumlah, update/hapus tanpa reload
- **Autentikasi**: registrasi & login email + password, guest checkout didukung
- **Checkout & pesanan**: order transaksional (anti-oversell, stok dikurangi saat order dibuat), riwayat pesanan
- **Pembayaran**: trait abstrak `PaymentGateway` + implementasi dummy untuk dev (webhook bertanda tangan); tinggal `impl` untuk gateway nyata
- **Panel admin**: CRUD produk (dengan upload gambar: resize + kompres otomatis), kelola pesanan (tandai lunas / batalkan + restore stok). Non-admin tak melihat panel (404)
- **Konfigurasi**: nama toko, admin, secret, bind address via environment variable

## Struktur

```
src/
├── main.rs           # entry: router, layer, startup
├── config.rs         # baca environment variable
├── db.rs             # koneksi + migrasi
├── state.rs          # AppState bersama
├── auth.rs           # hashing, session, guard admin
├── cart.rs           # keranjang session
├── uploads.rs        # simpan/hapus gambar produk (resize+kompres)
├── error.rs          # AppError -> HTTP response
├── handlers/         # catalog, cart, auth, order, payment, admin
├── models/           # product, user, order
├── payment/          # trait PaymentGateway + DummyGateway
└── templates/        # struct Askama + helper
templates/            # berkas .html
static/               # CSS, htmx.min.js, uploads/
migrations/           # 0001_init .. 0004_payment
```

## Prasyarat

- Rust (edition 2024 — toolchain terbaru)
- PostgreSQL yang bisa diakses

## Setup

1. Salin konfigurasi:
   ```sh
   cp .env.example .env
   ```
2. Isi `.env` (lihat tabel di bawah). `DATABASE_URL` wajib. Karakter spesial di password harus di-URL-encode (mis. `@` → `%40`).
3. Migrasi dijalankan **otomatis** saat startup — tak perlu langkah manual.

### Environment variable

| Variabel | Wajib | Default | Keterangan |
|---|---|---|---|
| `DATABASE_URL` | ✅ | — | `postgres://user:pass@host:5432/online_store` |
| `BIND_ADDR` | | `0.0.0.0:3000` | alamat listen |
| `STORE_NAME` | | `Toko Online` | nama toko (brand, judul tab) |
| `ADMIN_EMAIL` | | — | email akun yang dipromosikan jadi admin saat startup |
| `PAYMENT_DUMMY_SECRET` | | `dev-dummy-secret` | secret webhook gateway dummy (dev) |
| `RUST_LOG` | | `info` | level log |

## Build & Run

Build memerlukan `DATABASE_URL` di env (SQLx cek query saat compile — mode online):

```sh
# build
export $(grep '^DATABASE_URL=' .env) && cargo build

# run
export $(grep -v '^#' .env | grep -v '^$' | xargs -d '\n') && ./target/debug/online_store
```

Server berjalan di `BIND_ADDR` (default http://localhost:3000). Panel admin di `/admin` (perlu login dengan akun `ADMIN_EMAIL`).

## Catatan produksi

Sebelum deploy, wajib disesuaikan:

- **Cookie session `secure`**: di `main.rs` `with_secure(false)` → set `true` saat pakai HTTPS
- **SQLx offline**: untuk CI/deploy tanpa DB reachable, jalankan `cargo sqlx prepare` → commit `.sqlx/`, set `SQLX_OFFLINE=true`
- **Gateway pembayaran**: ganti `DummyGateway` dengan implementasi nyata + verifikasi signature asli; endpoint webhook wajib HTTPS
- **Hapus akun tes** dari database dev

## Lisensi

Belum ditentukan.
