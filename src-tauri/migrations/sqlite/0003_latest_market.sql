-- Materialized "latest market snapshot per (station, commodity)".
-- Route queries get a 10-100x speedup vs. DISTINCT ON over the full history.
CREATE TABLE latest_market (
    station_id   INTEGER NOT NULL,
    commodity_id INTEGER NOT NULL,
    buy_price    INTEGER,
    sell_price   INTEGER,
    supply       INTEGER NOT NULL DEFAULT 0,
    demand       INTEGER NOT NULL DEFAULT 0,
    recorded_at  TEXT NOT NULL,
    PRIMARY KEY (station_id, commodity_id)
);

-- Populate from existing history (idempotent via PK).
INSERT INTO latest_market (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at)
SELECT m.station_id, m.commodity_id, m.buy_price, m.sell_price, m.supply, m.demand, m.recorded_at
FROM market_snapshots m
INNER JOIN (
    SELECT station_id, commodity_id, MAX(recorded_at) AS max_rec
    FROM market_snapshots
    GROUP BY station_id, commodity_id
) latest
    ON latest.station_id = m.station_id
   AND latest.commodity_id = m.commodity_id
   AND latest.max_rec = m.recorded_at;

CREATE INDEX ix_latest_buy  ON latest_market(commodity_id, buy_price  DESC) WHERE buy_price  > 0 AND supply > 0;
CREATE INDEX ix_latest_sell ON latest_market(commodity_id, sell_price DESC) WHERE sell_price > 0 AND demand > 0;
CREATE INDEX ix_latest_station ON latest_market(station_id);
