# syntax=docker/dockerfile:1

# ============================================================
# Stage 1 — builder: kompilasi rilis dari sumber.
# ------------------------------------------------------------
# Edition 2024 butuh rustc >= 1.85; pin 1.93 (samakan dgn dev).
# rustls memakai backend `ring` -> perlu toolchain C (sudah ada
# di image rust). Tidak butuh OpenSSL sistem sama sekali.
FROM rust:1.93-bookworm AS builder

WORKDIR /app

# Build tanpa DB reachable: gunakan cache query hasil `cargo sqlx prepare`.
ENV SQLX_OFFLINE=true

# ---- Lapis cache dependensi (agar tak rebuild deps tiap ubah src) ----
# Salin manifest + lockfile + cache sqlx dulu, buat src dummy, build deps.
COPY Cargo.toml Cargo.lock ./
COPY .sqlx ./.sqlx
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src

# ---- Build aplikasi sebenarnya ----
# migrations/ & templates/ ikut dibaca compile-time (sqlx::migrate!, Askama).
COPY src ./src
COPY migrations ./migrations
COPY templates ./templates
COPY static ./static
# Sentuh main.rs agar cargo tahu sumber berubah dari dummy di atas.
RUN touch src/main.rs && cargo build --release

# ============================================================
# Stage 2 — runtime: image ramping, hanya binary + aset statis.
# ------------------------------------------------------------
# ca-certificates: verifikasi TLS saat konek Postgres (rustls pakai
# root CA sistem). Tanpa ini, koneksi TLS ke DB gagal.
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Jalan sebagai non-root (keamanan): buat user tanpa hak istimewa.
RUN useradd --create-home --uid 10001 appuser
WORKDIR /app

# Binary rilis + aset statis (CSS, favicon). Templates & migrasi sudah
# tertanam di binary, tak perlu disalin.
COPY --from=builder /app/target/release/online_store /usr/local/bin/online_store
COPY --from=builder /app/static ./static

# Folder unggahan foto produk ditulis saat runtime. Jadikan volume agar
# data foto persist lintas restart/redeploy container.
RUN mkdir -p static/uploads && chown -R appuser:appuser /app
VOLUME ["/app/static/uploads"]

USER appuser

# Default; timpa lewat -e BIND_ADDR / env compose bila perlu.
ENV BIND_ADDR=0.0.0.0:3000
EXPOSE 3000

# Runtime WAJIB env: DATABASE_URL (Postgres). Opsional: SECURE_COOKIES=true
# (produksi HTTPS), STORE_NAME, ADMIN_EMAIL, PAYMENT_*, RUST_LOG.
CMD ["online_store"]
