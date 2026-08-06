-- Batch 1 — skema katalog produk.
-- Uang: NUMERIC(12,2) -> dipetakan ke rust_decimal::Decimal.

CREATE TABLE products (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    price       NUMERIC(12, 2) NOT NULL CHECK (price >= 0),
    stock       INTEGER NOT NULL DEFAULT 0 CHECK (stock >= 0),
    image_url   TEXT,
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_products_is_active ON products (is_active);

-- Trigger: perbarui updated_at otomatis setiap UPDATE.
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_products_updated_at
    BEFORE UPDATE ON products
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

-- Seed data contoh supaya halaman langsung berisi.
INSERT INTO products (slug, name, description, price, stock, image_url) VALUES
    ('kaos-polos-hitam',   'Kaos Polos Hitam',    'Kaos katun combed 30s, adem dan nyaman dipakai harian.', 85000.00,  120, NULL),
    ('hoodie-abu',         'Hoodie Abu-abu',      'Hoodie fleece tebal dengan kantong depan, hangat.',      250000.00, 40,  NULL),
    ('topi-baseball',      'Topi Baseball',       'Topi baseball adjustable, bahan drill premium.',         65000.00,  75,  NULL),
    ('sepatu-sneakers',    'Sepatu Sneakers Putih','Sneakers kasual putih, sol karet anti-slip.',            325000.00, 25,  NULL);
