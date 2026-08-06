-- M4 — pesanan (orders) + item pesanan (order_items).

CREATE TABLE orders (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    number           BIGSERIAL NOT NULL UNIQUE,           -- nomor order ramah manusia
    user_id          UUID REFERENCES users(id) ON DELETE SET NULL, -- NULL untuk guest checkout
    email            TEXT NOT NULL,
    customer_name    TEXT NOT NULL,
    phone            TEXT NOT NULL DEFAULT '',
    shipping_address TEXT NOT NULL,
    status           TEXT NOT NULL DEFAULT 'pending'
                     CHECK (status IN ('pending', 'paid', 'cancelled')),
    subtotal         NUMERIC(12, 2) NOT NULL CHECK (subtotal >= 0),
    total            NUMERIC(12, 2) NOT NULL CHECK (total >= 0),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_orders_user_id ON orders (user_id);
CREATE INDEX idx_orders_status ON orders (status);

CREATE TRIGGER trg_orders_updated_at
    BEFORE UPDATE ON orders
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE order_items (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id     UUID NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    product_id   UUID REFERENCES products(id) ON DELETE SET NULL,
    product_name TEXT NOT NULL,             -- snapshot nama saat order
    unit_price   NUMERIC(12, 2) NOT NULL CHECK (unit_price >= 0), -- snapshot harga saat order
    qty          INTEGER NOT NULL CHECK (qty > 0),
    line_total   NUMERIC(12, 2) NOT NULL CHECK (line_total >= 0)
);

CREATE INDEX idx_order_items_order_id ON order_items (order_id);
