CREATE TABLE stations (
    station_id        INTEGER PRIMARY KEY,
    system_name       TEXT NOT NULL,
    system_id64       INTEGER,
    station_name      TEXT NOT NULL,
    pad_size          TEXT,
    station_type      TEXT,
    is_fleet_carrier  INTEGER NOT NULL DEFAULT 0,
    distance_to_star  REAL,
    services          TEXT,
    last_seen_at      TEXT NOT NULL
);
CREATE INDEX ix_stations_system  ON stations(system_name);
CREATE INDEX ix_stations_carrier ON stations(is_fleet_carrier) WHERE is_fleet_carrier = 1;

CREATE TABLE systems (
    id64             INTEGER PRIMARY KEY,
    name             TEXT NOT NULL,
    x                REAL,
    y                REAL,
    z                REAL,
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
    is_rare          INTEGER NOT NULL DEFAULT 0,
    is_illegal_hint  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE market_snapshots (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    station_id    INTEGER NOT NULL REFERENCES stations(station_id),
    commodity_id  INTEGER NOT NULL REFERENCES commodities(commodity_id),
    buy_price     INTEGER,
    sell_price    INTEGER,
    supply        INTEGER NOT NULL DEFAULT 0,
    demand        INTEGER NOT NULL DEFAULT 0,
    recorded_at   TEXT NOT NULL,
    ingested_at   TEXT NOT NULL DEFAULT (datetime('now')),
    source        TEXT NOT NULL,
    UNIQUE (station_id, commodity_id, recorded_at)
);
CREATE INDEX ix_snap_recorded          ON market_snapshots(recorded_at);
CREATE INDEX ix_snap_station_commodity ON market_snapshots(station_id, commodity_id, recorded_at DESC);
CREATE INDEX ix_snap_commodity_sell    ON market_snapshots(commodity_id, sell_price DESC) WHERE sell_price IS NOT NULL;

CREATE TABLE user_state (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    current_system  TEXT,
    current_station TEXT,
    ship_type       TEXT,
    cargo_capacity  INTEGER,
    jump_range_ly   REAL,
    credits         INTEGER,
    pad_size_max    TEXT,
    updated_at      TEXT NOT NULL
);

CREATE TABLE settings (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);

CREATE TABLE alert_log (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    route_hash     TEXT NOT NULL,
    profit_per_ton INTEGER NOT NULL,
    fired_at       TEXT NOT NULL,
    channel        TEXT NOT NULL
);
CREATE INDEX ix_alert_route ON alert_log(route_hash, fired_at DESC);
