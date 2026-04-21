CREATE TABLE stations (
    station_id        BIGINT PRIMARY KEY,
    system_name       TEXT NOT NULL,
    system_id64       BIGINT,
    station_name      TEXT NOT NULL,
    pad_size          TEXT,
    station_type      TEXT,
    is_fleet_carrier  BOOLEAN NOT NULL DEFAULT FALSE,
    distance_to_star  DOUBLE PRECISION,
    services          JSONB,
    last_seen_at      TIMESTAMPTZ NOT NULL
);
CREATE INDEX ix_stations_system  ON stations(system_name);
CREATE INDEX ix_stations_carrier ON stations(is_fleet_carrier) WHERE is_fleet_carrier;

CREATE TABLE systems (
    id64             BIGINT PRIMARY KEY,
    name             TEXT NOT NULL,
    x                DOUBLE PRECISION,
    y                DOUBLE PRECISION,
    z                DOUBLE PRECISION,
    allegiance       TEXT,
    government       TEXT,
    primary_economy  TEXT,
    security         TEXT
);
CREATE INDEX ix_systems_name   ON systems(name);
CREATE INDEX ix_systems_coords ON systems(x, y, z);

CREATE TABLE commodities (
    commodity_id     INTEGER PRIMARY KEY,
    symbol           TEXT UNIQUE NOT NULL,
    display_name     TEXT NOT NULL,
    category         TEXT,
    is_rare          BOOLEAN NOT NULL DEFAULT FALSE,
    is_illegal_hint  BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE market_snapshots (
    id            BIGSERIAL PRIMARY KEY,
    station_id    BIGINT NOT NULL REFERENCES stations(station_id),
    commodity_id  INTEGER NOT NULL REFERENCES commodities(commodity_id),
    buy_price     INTEGER,
    sell_price    INTEGER,
    supply        INTEGER NOT NULL DEFAULT 0,
    demand        INTEGER NOT NULL DEFAULT 0,
    recorded_at   TIMESTAMPTZ NOT NULL,
    ingested_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    source        TEXT NOT NULL,
    UNIQUE (station_id, commodity_id, recorded_at)
);
CREATE INDEX ix_snap_recorded          ON market_snapshots(recorded_at);
CREATE INDEX ix_snap_station_commodity ON market_snapshots(station_id, commodity_id, recorded_at DESC);
CREATE INDEX ix_snap_commodity_sell    ON market_snapshots(commodity_id, sell_price DESC) WHERE sell_price IS NOT NULL;

CREATE TABLE user_state (
    id              INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    current_system  TEXT,
    current_station TEXT,
    ship_type       TEXT,
    cargo_capacity  INTEGER,
    jump_range_ly   DOUBLE PRECISION,
    credits         BIGINT,
    pad_size_max    TEXT,
    updated_at      TIMESTAMPTZ NOT NULL
);

CREATE TABLE settings (
    key    TEXT PRIMARY KEY,
    value  JSONB NOT NULL
);

CREATE TABLE alert_log (
    id             BIGSERIAL PRIMARY KEY,
    route_hash     TEXT NOT NULL,
    profit_per_ton INTEGER NOT NULL,
    fired_at       TIMESTAMPTZ NOT NULL,
    channel        TEXT NOT NULL
);
CREATE INDEX ix_alert_route ON alert_log(route_hash, fired_at DESC);
