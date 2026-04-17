-- =============================================================================
-- db-viewer test schema
-- Covers: all MySQL data types, relationships, indexes, constraints
-- MySQL 8.0+
-- =============================================================================

DROP DATABASE IF EXISTS db_viewer_test;
CREATE DATABASE db_viewer_test
  CHARACTER SET utf8mb4
  COLLATE utf8mb4_unicode_ci;

USE db_viewer_test;

-- =============================================================================
-- 1. ALL DATA TYPES SHOWCASE
--    One table with every MySQL data type for visual testing
-- =============================================================================

CREATE TABLE all_types (
  -- Identity
  id                  BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,

  -- Integer types
  col_tinyint         TINYINT             NOT NULL DEFAULT 0,
  col_tinyint_u       TINYINT UNSIGNED    NOT NULL DEFAULT 0,
  col_smallint        SMALLINT            NOT NULL DEFAULT 0,
  col_smallint_u      SMALLINT UNSIGNED   NOT NULL DEFAULT 0,
  col_mediumint       MEDIUMINT           NOT NULL DEFAULT 0,
  col_mediumint_u     MEDIUMINT UNSIGNED  NOT NULL DEFAULT 0,
  col_int             INT                 NOT NULL DEFAULT 0,
  col_int_u           INT UNSIGNED        NOT NULL DEFAULT 0,
  col_bigint          BIGINT              NOT NULL DEFAULT 0,
  col_bigint_u        BIGINT UNSIGNED     NOT NULL DEFAULT 0,
  col_bit             BIT(8)              NOT NULL DEFAULT b'00000000',

  -- Boolean (alias of TINYINT(1))
  col_bool            BOOLEAN             NOT NULL DEFAULT FALSE,

  -- Floating point
  col_float           FLOAT               NULL,
  col_double          DOUBLE              NULL,
  col_decimal         DECIMAL(18, 6)      NULL,
  col_numeric         NUMERIC(10, 2)      NULL,

  -- Date & time
  col_date            DATE                NULL,
  col_datetime        DATETIME            NULL,
  col_datetime_fsp    DATETIME(6)         NULL,
  col_timestamp       TIMESTAMP           NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  col_timestamp_fsp   TIMESTAMP(3)        NULL,
  col_time            TIME                NULL,
  col_time_fsp        TIME(6)             NULL,
  col_year            YEAR                NULL,

  -- String types
  col_char            CHAR(10)            NULL,
  col_varchar         VARCHAR(255)        NULL,
  col_tinytext        TINYTEXT            NULL,
  col_text            TEXT                NULL,
  col_mediumtext      MEDIUMTEXT          NULL,
  col_longtext        LONGTEXT            NULL,

  -- Binary types
  col_binary          BINARY(16)          NULL,
  col_varbinary       VARBINARY(255)      NULL,
  col_tinyblob        TINYBLOB            NULL,
  col_blob            BLOB                NULL,
  col_mediumblob      MEDIUMBLOB          NULL,
  col_longblob        LONGBLOB            NULL,

  -- Enum & Set
  col_enum            ENUM('pending','active','inactive','deleted') NOT NULL DEFAULT 'pending',
  col_set             SET('read','write','delete','admin')          NOT NULL DEFAULT '',

  -- JSON
  col_json            JSON                NULL,

  -- Spatial
  col_geometry        GEOMETRY            NULL SRID 0,
  col_point           POINT               NULL SRID 0,
  col_linestring      LINESTRING          NULL SRID 0,
  col_polygon         POLYGON             NULL SRID 0,

  -- Generated column (stored)
  col_generated       VARCHAR(100)        GENERATED ALWAYS AS (CONCAT(col_char, ' — ', col_varchar)) STORED,

  PRIMARY KEY (id),
  INDEX idx_all_types_date    (col_date),
  INDEX idx_all_types_enum    (col_enum),
  INDEX idx_all_types_bool    (col_bool),
  FULLTEXT INDEX ft_all_types (col_text, col_varchar)
) ENGINE=InnoDB;


-- =============================================================================
-- 2. COUNTRIES  (simple lookup, no FKs)
-- =============================================================================

CREATE TABLE countries (
  id          SMALLINT UNSIGNED   NOT NULL AUTO_INCREMENT,
  code        CHAR(2)             NOT NULL,
  name        VARCHAR(100)        NOT NULL,
  phone_code  VARCHAR(10)         NULL,
  created_at  TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (id),
  UNIQUE KEY uq_countries_code (code)
) ENGINE=InnoDB;


-- =============================================================================
-- 3. CATEGORIES  (self-referential / tree)
-- =============================================================================

CREATE TABLE categories (
  id          INT UNSIGNED        NOT NULL AUTO_INCREMENT,
  parent_id   INT UNSIGNED        NULL,
  name        VARCHAR(100)        NOT NULL,
  slug        VARCHAR(120)        NOT NULL,
  depth       TINYINT UNSIGNED    NOT NULL DEFAULT 0,
  sort_order  SMALLINT            NOT NULL DEFAULT 0,
  active      BOOLEAN             NOT NULL DEFAULT TRUE,
  created_at  TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at  TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  PRIMARY KEY (id),
  UNIQUE KEY uq_categories_slug (slug),
  INDEX idx_categories_parent (parent_id),
  INDEX idx_categories_active (active),
  CONSTRAINT fk_categories_parent
    FOREIGN KEY (parent_id) REFERENCES categories (id)
    ON DELETE SET NULL ON UPDATE CASCADE
) ENGINE=InnoDB;


-- =============================================================================
-- 4. USERS
-- =============================================================================

CREATE TABLE users (
  id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
  country_id      SMALLINT UNSIGNED   NULL,
  username        VARCHAR(50)         NOT NULL,
  email           VARCHAR(254)        NOT NULL,
  password_hash   CHAR(60)            NOT NULL,
  first_name      VARCHAR(80)         NULL,
  last_name       VARCHAR(80)         NULL,
  bio             TEXT                NULL,
  avatar_url      VARCHAR(512)        NULL,
  role            ENUM('guest','user','editor','admin') NOT NULL DEFAULT 'user',
  status          ENUM('active','suspended','deleted')  NOT NULL DEFAULT 'active',
  score           DECIMAL(6,2)        NOT NULL DEFAULT 0.00,
  login_count     INT UNSIGNED        NOT NULL DEFAULT 0,
  last_login_at   DATETIME            NULL,
  email_verified  BOOLEAN             NOT NULL DEFAULT FALSE,
  verified_at     DATETIME            NULL,
  birth_date      DATE                NULL,
  preferences     JSON                NULL,
  created_at      TIMESTAMP(3)        NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  updated_at      TIMESTAMP(3)        NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3),
  deleted_at      DATETIME            NULL,

  PRIMARY KEY (id),
  UNIQUE KEY uq_users_email    (email),
  UNIQUE KEY uq_users_username (username),
  INDEX idx_users_country    (country_id),
  INDEX idx_users_role       (role),
  INDEX idx_users_status     (status),
  INDEX idx_users_created    (created_at),
  INDEX idx_users_deleted    (deleted_at),
  FULLTEXT INDEX ft_users    (first_name, last_name, bio),
  CONSTRAINT fk_users_country
    FOREIGN KEY (country_id) REFERENCES countries (id)
    ON DELETE SET NULL ON UPDATE CASCADE,
  CONSTRAINT chk_users_score CHECK (score >= 0)
) ENGINE=InnoDB;


-- =============================================================================
-- 5. PRODUCTS
-- =============================================================================

CREATE TABLE products (
  id              INT UNSIGNED        NOT NULL AUTO_INCREMENT,
  category_id     INT UNSIGNED        NULL,
  sku             VARCHAR(64)         NOT NULL,
  name            VARCHAR(200)        NOT NULL,
  slug            VARCHAR(220)        NOT NULL,
  description     MEDIUMTEXT          NULL,
  price           DECIMAL(12,2)       NOT NULL DEFAULT 0.00,
  compare_price   DECIMAL(12,2)       NULL,
  cost_price      DECIMAL(12,2)       NULL,
  stock           INT                 NOT NULL DEFAULT 0,
  weight_grams    MEDIUMINT UNSIGNED  NULL,
  status          ENUM('draft','published','archived') NOT NULL DEFAULT 'draft',
  tags            SET('sale','new','featured','bestseller','digital') NOT NULL DEFAULT '',
  meta            JSON                NULL,
  published_at    DATETIME            NULL,
  created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  PRIMARY KEY (id),
  UNIQUE KEY uq_products_sku  (sku),
  UNIQUE KEY uq_products_slug (slug),
  INDEX idx_products_category (category_id),
  INDEX idx_products_status   (status),
  INDEX idx_products_price    (price),
  INDEX idx_products_stock    (stock),
  FULLTEXT INDEX ft_products  (name, description),
  CONSTRAINT fk_products_category
    FOREIGN KEY (category_id) REFERENCES categories (id)
    ON DELETE SET NULL ON UPDATE CASCADE,
  CONSTRAINT chk_products_price CHECK (price >= 0),
  CONSTRAINT chk_products_stock CHECK (stock >= 0)
) ENGINE=InnoDB;


-- =============================================================================
-- 6. PRODUCT IMAGES  (1:N with products)
-- =============================================================================

CREATE TABLE product_images (
  id          INT UNSIGNED    NOT NULL AUTO_INCREMENT,
  product_id  INT UNSIGNED    NOT NULL,
  url         VARCHAR(512)    NOT NULL,
  alt_text    VARCHAR(255)    NULL,
  width       SMALLINT UNSIGNED NULL,
  height      SMALLINT UNSIGNED NULL,
  sort_order  TINYINT UNSIGNED  NOT NULL DEFAULT 0,
  is_cover    BOOLEAN           NOT NULL DEFAULT FALSE,
  created_at  TIMESTAMP         NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (id),
  INDEX idx_product_images_product (product_id),
  INDEX idx_product_images_cover   (product_id, is_cover),
  CONSTRAINT fk_product_images_product
    FOREIGN KEY (product_id) REFERENCES products (id)
    ON DELETE CASCADE ON UPDATE CASCADE
) ENGINE=InnoDB;


-- =============================================================================
-- 7. TAGS  (M:N with products via pivot)
-- =============================================================================

CREATE TABLE tags (
  id          SMALLINT UNSIGNED   NOT NULL AUTO_INCREMENT,
  name        VARCHAR(50)         NOT NULL,
  slug        VARCHAR(55)         NOT NULL,
  color       CHAR(7)             NULL COMMENT 'Hex color e.g. #ff0000',
  created_at  TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (id),
  UNIQUE KEY uq_tags_slug (slug)
) ENGINE=InnoDB;


CREATE TABLE product_tags (
  product_id  INT UNSIGNED        NOT NULL,
  tag_id      SMALLINT UNSIGNED   NOT NULL,
  created_at  TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (product_id, tag_id),
  INDEX idx_product_tags_tag (tag_id),
  CONSTRAINT fk_product_tags_product
    FOREIGN KEY (product_id) REFERENCES products (id)
    ON DELETE CASCADE ON UPDATE CASCADE,
  CONSTRAINT fk_product_tags_tag
    FOREIGN KEY (tag_id) REFERENCES tags (id)
    ON DELETE CASCADE ON UPDATE CASCADE
) ENGINE=InnoDB;


-- =============================================================================
-- 8. ADDRESSES  (polymorphic-style: users can have many)
-- =============================================================================

CREATE TABLE addresses (
  id          INT UNSIGNED        NOT NULL AUTO_INCREMENT,
  user_id     BIGINT UNSIGNED     NOT NULL,
  country_id  SMALLINT UNSIGNED   NULL,
  label       ENUM('home','work','other') NOT NULL DEFAULT 'home',
  line1       VARCHAR(200)        NOT NULL,
  line2       VARCHAR(200)        NULL,
  city        VARCHAR(100)        NOT NULL,
  state       VARCHAR(100)        NULL,
  postal_code VARCHAR(20)         NULL,
  lat         DECIMAL(10,7)       NULL,
  lng         DECIMAL(10,7)       NULL,
  is_default  BOOLEAN             NOT NULL DEFAULT FALSE,
  created_at  TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (id),
  INDEX idx_addresses_user    (user_id),
  INDEX idx_addresses_country (country_id),
  CONSTRAINT fk_addresses_user
    FOREIGN KEY (user_id) REFERENCES users (id)
    ON DELETE CASCADE ON UPDATE CASCADE,
  CONSTRAINT fk_addresses_country
    FOREIGN KEY (country_id) REFERENCES countries (id)
    ON DELETE SET NULL ON UPDATE CASCADE
) ENGINE=InnoDB;


-- =============================================================================
-- 9. ORDERS
-- =============================================================================

CREATE TABLE orders (
  id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
  user_id         BIGINT UNSIGNED     NOT NULL,
  address_id      INT UNSIGNED        NULL,
  order_number    VARCHAR(32)         NOT NULL,
  status          ENUM('pending','confirmed','processing','shipped','delivered','cancelled','refunded')
                                      NOT NULL DEFAULT 'pending',
  currency        CHAR(3)             NOT NULL DEFAULT 'EUR',
  subtotal        DECIMAL(12,2)       NOT NULL DEFAULT 0.00,
  discount        DECIMAL(12,2)       NOT NULL DEFAULT 0.00,
  tax             DECIMAL(12,2)       NOT NULL DEFAULT 0.00,
  shipping        DECIMAL(12,2)       NOT NULL DEFAULT 0.00,
  total           DECIMAL(12,2)       NOT NULL DEFAULT 0.00,
  notes           TEXT                NULL,
  metadata        JSON                NULL,
  confirmed_at    DATETIME            NULL,
  shipped_at      DATETIME            NULL,
  delivered_at    DATETIME            NULL,
  cancelled_at    DATETIME            NULL,
  created_at      TIMESTAMP(6)        NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at      TIMESTAMP(6)        NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),

  PRIMARY KEY (id),
  UNIQUE KEY uq_orders_number (order_number),
  INDEX idx_orders_user    (user_id),
  INDEX idx_orders_address (address_id),
  INDEX idx_orders_status  (status),
  INDEX idx_orders_created (created_at),
  CONSTRAINT fk_orders_user
    FOREIGN KEY (user_id) REFERENCES users (id)
    ON DELETE RESTRICT ON UPDATE CASCADE,
  CONSTRAINT fk_orders_address
    FOREIGN KEY (address_id) REFERENCES addresses (id)
    ON DELETE SET NULL ON UPDATE CASCADE,
  CONSTRAINT chk_orders_total CHECK (total >= 0)
) ENGINE=InnoDB;


-- =============================================================================
-- 10. ORDER ITEMS
-- =============================================================================

CREATE TABLE order_items (
  id          BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
  order_id    BIGINT UNSIGNED     NOT NULL,
  product_id  INT UNSIGNED        NULL,
  sku         VARCHAR(64)         NOT NULL,
  name        VARCHAR(200)        NOT NULL,
  quantity    SMALLINT UNSIGNED   NOT NULL DEFAULT 1,
  unit_price  DECIMAL(12,2)       NOT NULL,
  discount    DECIMAL(12,2)       NOT NULL DEFAULT 0.00,
  tax_rate    DECIMAL(5,4)        NOT NULL DEFAULT 0.0000,
  subtotal    DECIMAL(12,2)       NOT NULL,
  meta        JSON                NULL,

  PRIMARY KEY (id),
  INDEX idx_order_items_order   (order_id),
  INDEX idx_order_items_product (product_id),
  CONSTRAINT fk_order_items_order
    FOREIGN KEY (order_id) REFERENCES orders (id)
    ON DELETE CASCADE ON UPDATE CASCADE,
  CONSTRAINT fk_order_items_product
    FOREIGN KEY (product_id) REFERENCES products (id)
    ON DELETE SET NULL ON UPDATE CASCADE,
  CONSTRAINT chk_order_items_qty CHECK (quantity > 0)
) ENGINE=InnoDB;


-- =============================================================================
-- 11. COUPONS
-- =============================================================================

CREATE TABLE coupons (
  id              INT UNSIGNED        NOT NULL AUTO_INCREMENT,
  code            VARCHAR(32)         NOT NULL,
  description     VARCHAR(255)        NULL,
  type            ENUM('percent','fixed','free_shipping') NOT NULL DEFAULT 'percent',
  value           DECIMAL(10,2)       NOT NULL DEFAULT 0.00,
  min_order       DECIMAL(10,2)       NULL,
  max_uses        INT UNSIGNED        NULL COMMENT 'NULL = unlimited',
  used_count      INT UNSIGNED        NOT NULL DEFAULT 0,
  active          BOOLEAN             NOT NULL DEFAULT TRUE,
  starts_at       DATETIME            NULL,
  expires_at      DATETIME            NULL,
  created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (id),
  UNIQUE KEY uq_coupons_code (code),
  INDEX idx_coupons_active  (active),
  INDEX idx_coupons_expires (expires_at)
) ENGINE=InnoDB;


-- =============================================================================
-- 12. EVENT LOG  (append-only, various text/blob/json)
-- =============================================================================

CREATE TABLE event_log (
  id          BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
  user_id     BIGINT UNSIGNED     NULL,
  event       VARCHAR(100)        NOT NULL,
  level       ENUM('debug','info','warning','error','critical') NOT NULL DEFAULT 'info',
  message     TEXT                NULL,
  context     JSON                NULL,
  ip_address  VARCHAR(45)         NULL COMMENT 'IPv4 or IPv6',
  user_agent  VARCHAR(512)        NULL,
  duration_ms INT UNSIGNED        NULL COMMENT 'Request duration in ms',
  created_at  TIMESTAMP(6)        NOT NULL DEFAULT CURRENT_TIMESTAMP(6),

  PRIMARY KEY (id),
  INDEX idx_event_log_user    (user_id),
  INDEX idx_event_log_event   (event),
  INDEX idx_event_log_level   (level),
  INDEX idx_event_log_created (created_at),
  CONSTRAINT fk_event_log_user
    FOREIGN KEY (user_id) REFERENCES users (id)
    ON DELETE SET NULL ON UPDATE CASCADE
) ENGINE=InnoDB;


-- =============================================================================
-- 13. SCHEDULES  (TIME and YEAR focus)
-- =============================================================================

CREATE TABLE schedules (
  id          INT UNSIGNED        NOT NULL AUTO_INCREMENT,
  name        VARCHAR(100)        NOT NULL,
  day_of_week TINYINT UNSIGNED    NOT NULL COMMENT '0=Sunday … 6=Saturday',
  start_time  TIME                NOT NULL,
  end_time    TIME                NOT NULL,
  valid_from  YEAR                NULL,
  valid_until YEAR                NULL,
  active      BOOLEAN             NOT NULL DEFAULT TRUE,
  created_at  TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (id),
  INDEX idx_schedules_day (day_of_week)
) ENGINE=InnoDB;


-- =============================================================================
-- 14. COMPOSITE INDEX & COVERING INDEX demo
-- =============================================================================

CREATE TABLE page_views (
  id          BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
  user_id     BIGINT UNSIGNED     NULL,
  session_id  CHAR(36)            NOT NULL,
  path        VARCHAR(512)        NOT NULL,
  referrer    VARCHAR(512)        NULL,
  duration_s  SMALLINT UNSIGNED   NULL,
  viewed_at   DATETIME(3)         NOT NULL,

  PRIMARY KEY (id),
  INDEX idx_pv_user_date  (user_id, viewed_at),      -- composite
  INDEX idx_pv_session    (session_id),
  INDEX idx_pv_path_date  (path(100), viewed_at),    -- prefix + date covering
  CONSTRAINT fk_pv_user
    FOREIGN KEY (user_id) REFERENCES users (id)
    ON DELETE SET NULL ON UPDATE CASCADE
) ENGINE=InnoDB;


-- =============================================================================
-- DATA
-- =============================================================================

-- Countries
INSERT INTO countries (code, name, phone_code) VALUES
  ('ES', 'Spain',          '+34'),
  ('US', 'United States',  '+1'),
  ('MX', 'Mexico',         '+52'),
  ('DE', 'Germany',        '+49'),
  ('JP', 'Japan',          '+81'),
  ('BR', 'Brazil',         '+55'),
  ('FR', 'France',         '+33'),
  ('GB', 'United Kingdom', '+44');

-- Categories
INSERT INTO categories (parent_id, name, slug, depth, sort_order) VALUES
  (NULL, 'Electronics',       'electronics',        0, 1),
  (NULL, 'Clothing',          'clothing',           0, 2),
  (NULL, 'Books',             'books',              0, 3),
  (1,    'Smartphones',       'smartphones',        1, 1),
  (1,    'Laptops',           'laptops',            1, 2),
  (1,    'Audio',             'audio',              1, 3),
  (2,    'Men',               'clothing-men',       1, 1),
  (2,    'Women',             'clothing-women',     1, 2),
  (4,    'Android',           'android-phones',     2, 1),
  (4,    'iPhone',            'iphones',            2, 2);

-- Users
INSERT INTO users (country_id, username, email, password_hash, first_name, last_name, bio, role, status, score, login_count, last_login_at, email_verified, verified_at, birth_date, preferences, created_at, updated_at) VALUES
  (1, 'ivan_g',     'ivan@example.com',   '$2b$10$abc123', 'Iván',    'García',   'Full stack developer based in Spain.',    'admin',  'active',    99.50, 142, '2024-03-15 09:23:11', TRUE,  '2023-01-10 10:00:00', '1990-05-14', '{"theme":"dark","lang":"es","notifications":true}',  '2023-01-10 10:00:00', NOW()),
  (2, 'alice_w',    'alice@example.com',  '$2b$10$def456', 'Alice',   'Walker',   'Designer and coffee enthusiast.',         'editor', 'active',    78.25,  89, '2024-03-14 18:05:00', TRUE,  '2023-02-20 14:30:00', '1985-11-23', '{"theme":"light","lang":"en","notifications":false}', '2023-02-20 14:30:00', NOW()),
  (3, 'carlos_m',  'carlos@example.com', '$2b$10$ghi789', 'Carlos',  'Mendoza',  NULL,                                      'user',   'active',    45.00,  23, '2024-02-28 11:00:00', TRUE,  '2023-06-01 08:00:00', '1995-03-07', NULL,                                                 '2023-06-01 08:00:00', NOW()),
  (4, 'hans_m',    'hans@example.com',   '$2b$10$jkl012', 'Hans',    'Müller',   'Engineer. Linux user since 2001.',        'user',   'active',    60.10,  55, '2024-03-10 07:45:00', TRUE,  '2023-03-15 09:00:00', '1988-07-19', '{"theme":"dark","lang":"de"}',                        '2023-03-15 09:00:00', NOW()),
  (5, 'yuki_t',    'yuki@example.com',   '$2b$10$mno345', 'Yuki',    'Tanaka',   'Manga artist and web dev.',               'user',   'active',    91.80,  210,'2024-03-16 02:10:00', TRUE,  '2022-12-01 00:00:00', '1993-09-30', '{"theme":"auto","lang":"ja"}',                        '2022-12-01 00:00:00', NOW()),
  (1, 'pepita_r',  'pepita@example.com', '$2b$10$pqr678', 'Pepita',  'Ruiz',     NULL,                                      'user',   'suspended', 10.00,   3, NULL,                  FALSE, NULL,                  NULL,         NULL,                                                 '2024-01-05 16:20:00', NOW()),
  (2, 'bob_d',     'bob@example.com',    '$2b$10$stu901', 'Bob',     'Doe',      'Just a regular user.',                    'user',   'active',    33.33,  17, '2024-03-01 20:00:00', TRUE,  '2024-01-15 12:00:00', '2000-01-01', '{"theme":"light"}',                                  '2024-01-15 12:00:00', NOW()),
  (NULL, 'ghost',  'ghost@example.com',  '$2b$10$vwx234', NULL,      NULL,       NULL,                                      'guest',  'deleted',    0.00,   1, NULL,                  FALSE, NULL,                  NULL,         NULL,                                                 '2024-03-01 00:00:00', NOW());

-- Tags
INSERT INTO tags (name, slug, color) VALUES
  ('Sale',       'sale',       '#ef4444'),
  ('New',        'new',        '#3b82f6'),
  ('Featured',   'featured',   '#f59e0b'),
  ('Eco',        'eco',        '#22c55e'),
  ('Limited',    'limited',    '#a855f7');

-- Products
INSERT INTO products (category_id, sku, name, slug, description, price, compare_price, cost_price, stock, weight_grams, status, tags, meta, published_at) VALUES
  (5,  'LAP-001', 'ProBook X1 14"',      'probook-x1-14',       'Professional laptop with Intel Core i7, 16GB RAM, 512GB SSD.',           1299.00, 1499.00,  800.00, 45,  1850, 'published', 'featured,new',  '{"brand":"ProBook","warranty_years":2}', '2024-01-15 09:00:00'),
  (5,  'LAP-002', 'UltraSlim Z5',        'ultraslim-z5',        'Lightweight 13" laptop, 8GB RAM, 256GB SSD. Perfect for travel.',         899.00,  NULL,     550.00, 12,  1200, 'published', 'sale',          '{"brand":"UltraSlim","color":"silver"}',  '2024-02-01 10:00:00'),
  (9,  'PHO-001', 'Galaxy Note 22',      'galaxy-note-22',      'Latest Android flagship with 200MP camera and S-Pen.',                   999.00, 1099.00,  600.00, 78,   195, 'published', 'new,featured',  '{"brand":"Samsung","ram_gb":12}',        '2024-03-01 00:00:00'),
  (10, 'PHO-002', 'iPhone 16 Pro',       'iphone-16-pro',       'Apple iPhone 16 Pro, 256GB, Titanium finish.',                          1199.00, NULL,     750.00,  5,   187, 'published', 'featured',      '{"brand":"Apple","storage_gb":256}',     '2024-03-10 00:00:00'),
  (6,  'AUD-001', 'SoundMax BT500',      'soundmax-bt500',      'Over-ear wireless headphones, 40h battery, active noise cancellation.',   249.00,  299.00,  120.00, 120,  310, 'published', 'sale,bestseller','{"brand":"SoundMax","color":"black"}',   '2023-11-20 00:00:00'),
  (6,  'AUD-002', 'TinyBuds Pro',        'tinybud-pro',         'True wireless earbuds with spatial audio and IPX5 water resistance.',     179.00,  NULL,     90.00,  200,   55, 'published', 'new',           '{"brand":"TinyBuds"}',                   '2024-02-14 00:00:00'),
  (7,  'CLO-001', 'Classic Oxford Shirt','classic-oxford-shirt', 'Cotton Oxford shirt. Slim fit. Available in multiple colors.',            59.90,   79.90,   25.00, 300,  250, 'published', '',              '{"sizes":["S","M","L","XL"]}',           '2023-09-01 00:00:00'),
  (3,  'BOK-001', 'SQL for Humans',      'sql-for-humans',      'Comprehensive guide to SQL from beginner to advanced.',                    34.99,  NULL,      12.00, 500,  420, 'published', 'bestseller',    '{"author":"J. Smith","pages":480}',      '2022-06-10 00:00:00'),
  (3,  'BOK-002', 'Clean Architecture',  'clean-architecture',  NULL,                                                                       39.99,  NULL,      15.00, 250,  380, 'published', '',              '{"author":"R.C. Martin","pages":432}',   '2020-01-01 00:00:00'),
  (5,  'LAP-003', 'DevStation Pro',      'devstation-pro',      'Workstation laptop. 32GB RAM, 1TB NVMe, RTX 4080.',                      2499.00, NULL,    1500.00,   3,  2600, 'draft',     'new',           '{"brand":"DevStation"}',                 NULL);

-- Product ↔ Tags (M:N)
INSERT INTO product_tags (product_id, tag_id) VALUES
  (1, 2), (1, 3),  -- ProBook: New, Featured
  (2, 1),          -- UltraSlim: Sale
  (3, 2), (3, 3),  -- Galaxy: New, Featured
  (4, 3),          -- iPhone: Featured
  (5, 1), (5, 3),  -- SoundMax: Sale, Featured
  (6, 2),          -- TinyBuds: New
  (8, 3),          -- SQL Book: Featured
  (10,2), (10,5);  -- DevStation: New, Limited

-- Product images
INSERT INTO product_images (product_id, url, alt_text, width, height, sort_order, is_cover) VALUES
  (1, 'https://example.com/imgs/lap001-front.jpg',  'ProBook X1 front view',    1200, 800, 0, TRUE),
  (1, 'https://example.com/imgs/lap001-side.jpg',   'ProBook X1 side view',     1200, 800, 1, FALSE),
  (3, 'https://example.com/imgs/pho001-front.jpg',  'Galaxy Note 22',           800,  800, 0, TRUE),
  (5, 'https://example.com/imgs/aud001-main.jpg',   'SoundMax BT500',           900,  900, 0, TRUE),
  (5, 'https://example.com/imgs/aud001-box.jpg',    'SoundMax BT500 packaging', 900,  600, 1, FALSE);

-- Addresses
INSERT INTO addresses (user_id, country_id, label, line1, city, state, postal_code, lat, lng, is_default) VALUES
  (1, 1, 'home',  'Calle Gran Vía 42',        'Madrid',       'Community of Madrid', '28013', 40.4200270, -3.7024255, TRUE),
  (1, 1, 'work',  'Av. Diagonal 123',         'Barcelona',    'Catalonia',           '08018', 41.3927754,  2.1698983, FALSE),
  (2, 2, 'home',  '123 Main St',              'New York',     'NY',                  '10001', 40.7484405, -73.9967207,TRUE),
  (3, 3, 'home',  'Av. Insurgentes Sur 1000', 'Mexico City',  'CDMX',                '03100', 19.3910063, -99.1785250,TRUE),
  (4, 4, 'home',  'Unter den Linden 77',      'Berlin',       'Berlin',              '10117', 52.5170365, 13.3888599, TRUE),
  (5, 5, 'home',  '1-1 Shinjuku',             'Tokyo',        'Tokyo',               '160-0022',35.6938070,139.7034510,TRUE);

-- Orders
INSERT INTO orders (user_id, address_id, order_number, status, currency, subtotal, discount, tax, shipping, total, notes, confirmed_at, shipped_at, delivered_at, created_at) VALUES
  (1, 1, 'ORD-2024-0001', 'delivered',  'EUR', 1299.00,   0.00, 272.79,  0.00, 1571.79, NULL,              '2024-01-16 10:00:00', '2024-01-18 08:00:00', '2024-01-20 14:30:00', '2024-01-15 23:05:10.123456'),
  (1, 1, 'ORD-2024-0002', 'shipped',    'EUR',  249.00,  24.90,  47.07,  5.90,  277.07, 'Leave at door.',  '2024-03-12 09:00:00', '2024-03-14 07:00:00', NULL,                  '2024-03-11 18:22:05.000000'),
  (2, 3, 'ORD-2024-0003', 'confirmed',  'USD', 1199.00,   0.00, 107.91, 15.00, 1321.91, NULL,              '2024-03-15 11:30:00', NULL,                  NULL,                  '2024-03-15 10:55:33.500000'),
  (3, 4, 'ORD-2024-0004', 'pending',    'MXN',   34.99,   0.00,   5.60,  50.00,   90.59, NULL,             NULL,                  NULL,                  NULL,                  '2024-03-16 02:10:00.000000'),
  (5, 6, 'ORD-2024-0005', 'cancelled',  'JPY', 179.00,    0.00,  14.32,  0.00,  193.32, 'Changed my mind.', NULL,                 NULL,                  NULL,                  '2024-03-10 08:00:00.000000'),
  (4, 5, 'ORD-2024-0006', 'processing', 'EUR', 2498.99,  250.00, 449.82, 0.00, 2698.81, 'Business invoice.','2024-03-16 09:00:00', NULL,                  NULL,                  '2024-03-16 08:45:00.000000');

-- Order items
INSERT INTO order_items (order_id, product_id, sku, name, quantity, unit_price, discount, tax_rate, subtotal) VALUES
  (1, 1, 'LAP-001', 'ProBook X1 14"',      1, 1299.00,  0.00, 0.21, 1299.00),
  (2, 5, 'AUD-001', 'SoundMax BT500',      1,  249.00, 24.90, 0.21,  224.10),
  (3, 4, 'PHO-002', 'iPhone 16 Pro',       1, 1199.00,  0.00, 0.09, 1199.00),
  (4, 8, 'BOK-001', 'SQL for Humans',      1,   34.99,  0.00, 0.16,   34.99),
  (5, 6, 'AUD-002', 'TinyBuds Pro',        1,  179.00,  0.00, 0.08,  179.00),
  (6, 1, 'LAP-001', 'ProBook X1 14"',      1, 1299.00,  0.00, 0.21, 1299.00),
  (6, 5, 'AUD-001', 'SoundMax BT500',      1,  249.00, 250.00,0.21,    0.00),
  (6, 8, 'BOK-001', 'SQL for Humans',      2,   34.99,  0.00, 0.21,   69.98),
  (6, 9, 'BOK-002', 'Clean Architecture',  1,   39.99,  0.00, 0.21,   39.99);

-- Coupons
INSERT INTO coupons (code, description, type, value, min_order, max_uses, used_count, active, starts_at, expires_at) VALUES
  ('WELCOME10',  '10% off for new users',        'percent',      10.00,   0.00, 1000,   42, TRUE,  '2024-01-01 00:00:00', '2024-12-31 23:59:59'),
  ('SAVE50EUR',  '€50 off orders over €500',     'fixed',        50.00, 500.00,  500,   18, TRUE,  '2024-03-01 00:00:00', '2024-06-30 23:59:59'),
  ('FREESHIP',   'Free shipping on any order',   'free_shipping',  0.00,   0.00, NULL,   89, TRUE,  NULL,                  NULL),
  ('EXPIRED20',  '20% off — expired',            'percent',      20.00,   0.00,  100,  100, FALSE, '2023-01-01 00:00:00', '2023-12-31 23:59:59');

-- Schedules
INSERT INTO schedules (name, day_of_week, start_time, end_time, valid_from, valid_until, active) VALUES
  ('Morning shift',   1, '08:00:00', '12:00:00', 2024, NULL,  TRUE),
  ('Afternoon shift', 1, '13:00:00', '17:00:00', 2024, NULL,  TRUE),
  ('Night shift',     2, '22:00:00', '06:00:00', 2023, 2024,  FALSE),
  ('Weekend AM',      6, '09:00:00', '14:00:00', 2024, NULL,  TRUE),
  ('Weekend PM',      0, '09:00:00', '14:00:00', 2024, NULL,  TRUE);

-- Event log
INSERT INTO event_log (user_id, event, level, message, context, ip_address, user_agent, duration_ms) VALUES
  (1, 'user.login',       'info',    'User logged in successfully.',          '{"method":"password"}',             '88.12.34.56',  'Mozilla/5.0 (X11; Linux x86_64)', 45),
  (2, 'user.login',       'info',    'User logged in successfully.',          '{"method":"google_oauth"}',         '192.168.1.10', 'Chrome/123',                      120),
  (1, 'order.created',    'info',    'Order ORD-2024-0001 created.',          '{"order_id":1,"total":1571.79}',    '88.12.34.56',  'Mozilla/5.0 (X11; Linux x86_64)', 312),
  (3, 'user.register',    'info',    'New user registered.',                  '{"referral":null}',                 '189.200.1.99', 'Firefox/124',                     88),
  (6, 'user.login',       'warning', 'Failed login attempt.',                 '{"attempts":3}',                   '1.2.3.4',      'curl/7.88',                        12),
  (NULL,'cron.cleanup',   'debug',   'Old sessions cleaned up.',              '{"deleted":47}',                    NULL,           NULL,                               203),
  (1, 'product.viewed',   'debug',   'Product viewed.',                       '{"product_id":4}',                  '88.12.34.56',  'Mozilla/5.0',                     28),
  (5, 'order.cancelled',  'info',    'Order ORD-2024-0005 cancelled.',        '{"order_id":5,"reason":"user"}',    '203.0.113.5',  'Safari/17',                       95),
  (NULL,'system.error',   'error',   'Unexpected exception in payment gateway.','{"exception":"TimeoutException"}','127.0.0.1',    NULL,                               5001),
  (4, 'user.login',       'info',    'User logged in successfully.',          '{"method":"password"}',             '176.9.0.1',    'Mozilla/5.0 (Windows NT 10.0)',   67);

-- Page views
INSERT INTO page_views (user_id, session_id, path, referrer, duration_s, viewed_at) VALUES
  (1, '550e8400-e29b-41d4-a716-446655440000', '/',                    NULL,                        12, '2024-03-16 09:01:00.000'),
  (1, '550e8400-e29b-41d4-a716-446655440000', '/products',            '/',                         34, '2024-03-16 09:01:15.234'),
  (1, '550e8400-e29b-41d4-a716-446655440000', '/products/iphone-16-pro','https://google.com',      87, '2024-03-16 09:02:00.100'),
  (2, 'aaaabbbb-cccc-dddd-eeee-ffffffffffff', '/products/soundmax-bt500',NULL,                     55, '2024-03-15 14:22:10.500'),
  (NULL,'11111111-2222-3333-4444-555555555555','/products/sql-for-humans','https://twitter.com',  102, '2024-03-16 00:05:33.000');

-- All types: one rich row covering every column
INSERT INTO all_types (
  col_tinyint, col_tinyint_u, col_smallint, col_smallint_u, col_mediumint, col_mediumint_u,
  col_int, col_int_u, col_bigint, col_bigint_u, col_bit,
  col_bool,
  col_float, col_double, col_decimal, col_numeric,
  col_date, col_datetime, col_datetime_fsp, col_timestamp, col_timestamp_fsp,
  col_time, col_time_fsp, col_year,
  col_char, col_varchar,
  col_tinytext, col_text, col_mediumtext, col_longtext,
  col_binary, col_varbinary,
  col_tinyblob, col_blob, col_mediumblob,
  col_enum, col_set,
  col_json,
  col_geometry, col_point, col_linestring, col_polygon
) VALUES (
  -128, 255, -32768, 65535, -8388608, 16777215,
  -2147483648, 4294967295, -9223372036854775808, 18446744073709551615, b'10101010',
  TRUE,
  3.14159, 2.718281828459045, 123456789.123456, 9999.99,
  '2024-03-16', '2024-03-16 12:34:56', '2024-03-16 12:34:56.123456', '2024-03-16 12:34:56', '2024-03-16 12:34:56.123',
  '08:30:00', '08:30:00.123456', 2024,
  'HELLO', 'The quick brown fox jumps over the lazy dog',
  'Tiny text value', 'Regular text with some content here.', 'Medium text, could be longer.', 'Long text field for storing large amounts of content.',
  0x48656C6C6F, 0x576F726C64,
  'blob1', 'blob data here', 'medium blob',
  'active', 'read,write',
  '{"key":"value","number":42,"array":[1,2,3],"nested":{"bool":true,"null_val":null}}',
  ST_GeomFromText('POINT(40.4168 -3.7038)'),
  ST_GeomFromText('POINT(40.4168 -3.7038)'),
  ST_GeomFromText('LINESTRING(0 0, 1 1, 2 2)'),
  ST_GeomFromText('POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))')
);

-- A second row with many NULLs to test null rendering
INSERT INTO all_types (col_tinyint, col_bool, col_enum, col_set, col_varchar, col_date, col_year)
VALUES (0, FALSE, 'pending', '', 'Row with mostly NULLs', NULL, NULL);

-- A third row with edge-case values
INSERT INTO all_types (
  col_tinyint, col_tinyint_u, col_smallint, col_int, col_bigint,
  col_bool, col_float, col_double, col_decimal,
  col_date, col_datetime, col_time, col_year,
  col_char, col_varchar, col_text,
  col_enum, col_set, col_json
) VALUES (
  0, 0, 0, 0, 0,
  FALSE, 0.0, 0.0, 0.000000,
  '1000-01-01', '1000-01-01 00:00:00', '-838:59:59', 1901,
  '', '',  'Empty strings and minimum date/time values.',
  'deleted', 'admin', 'null'
);
