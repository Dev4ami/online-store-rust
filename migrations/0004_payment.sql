-- M5 — pembayaran. Simpan jejak bayar sebagai kolom di `orders` (1 pembayaran/order).
-- Status memakai kolom `status` + CHECK yang sudah ada (pending/paid/cancelled).

ALTER TABLE orders
    ADD COLUMN payment_method TEXT,        -- nama gateway/metode, mis. 'dummy', 'midtrans'
    ADD COLUMN payment_ref    TEXT,        -- referensi transaksi dari gateway
    ADD COLUMN paid_at        TIMESTAMPTZ; -- diisi saat lunas

CREATE INDEX idx_orders_payment_ref ON orders (payment_ref);
