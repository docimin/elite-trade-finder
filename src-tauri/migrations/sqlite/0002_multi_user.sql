-- Namespace per-install data by user_id so multiple installs can share one DB.
-- Shared tables (stations, systems, commodities, market_snapshots) are left alone.

DROP TABLE user_state;
CREATE TABLE user_state (
    user_id         TEXT PRIMARY KEY,
    current_system  TEXT,
    current_station TEXT,
    ship_type       TEXT,
    cargo_capacity  INTEGER,
    jump_range_ly   REAL,
    credits         INTEGER,
    pad_size_max    TEXT,
    updated_at      TEXT NOT NULL
);

DROP TABLE settings;
CREATE TABLE settings (
    user_id TEXT NOT NULL,
    key     TEXT NOT NULL,
    value   TEXT NOT NULL,
    PRIMARY KEY (user_id, key)
);

DROP INDEX IF EXISTS ix_alert_route;
ALTER TABLE alert_log ADD COLUMN user_id TEXT NOT NULL DEFAULT '';
CREATE INDEX ix_alert_user_route ON alert_log(user_id, route_hash, fired_at DESC);
