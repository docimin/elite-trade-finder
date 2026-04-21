-- Materialized "latest market snapshot per (station, commodity)".
CREATE TABLE latest_market (
    station_id   BIGINT NOT NULL,
    commodity_id INTEGER NOT NULL,
    buy_price    INTEGER,
    sell_price   INTEGER,
    supply       INTEGER NOT NULL DEFAULT 0,
    demand       INTEGER NOT NULL DEFAULT 0,
    recorded_at  TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (station_id, commodity_id)
);

INSERT INTO latest_market (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at)
SELECT DISTINCT ON (m.station_id, m.commodity_id)
       m.station_id, m.commodity_id, m.buy_price, m.sell_price, m.supply, m.demand, m.recorded_at
FROM market_snapshots m
ORDER BY m.station_id, m.commodity_id, m.recorded_at DESC;

CREATE INDEX ix_latest_buy  ON latest_market(commodity_id, buy_price  DESC) WHERE buy_price  > 0 AND supply > 0;
CREATE INDEX ix_latest_sell ON latest_market(commodity_id, sell_price DESC) WHERE sell_price > 0 AND demand > 0;
CREATE INDEX ix_latest_station ON latest_market(station_id);
