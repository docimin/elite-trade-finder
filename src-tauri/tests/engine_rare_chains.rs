use chrono::Utc;
use elite_trade_finder_lib::db;
use elite_trade_finder_lib::engine::rare_chains;
use elite_trade_finder_lib::types::ScoreWeights;
use tempfile::TempDir;

#[tokio::test]
async fn builds_rare_tour_to_distant_hub() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();
    db::seed::commodities(&conn).await.unwrap();

    let pool = match &conn {
        db::Db::Sqlite(p) => p.clone(),
        _ => panic!(),
    };
    sqlx::query("INSERT INTO systems (id64, name, x, y, z) VALUES (1, 'Rare1', 0,0,0), (2, 'Rare2', 3,0,0), (3, 'DistantHub', 200,0,0)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO stations (station_id, system_name, station_name, pad_size, last_seen_at) VALUES (100, 'Rare1', 'Onion Farm', 'L', ?), (200, 'Rare2', 'Other Rare', 'L', ?), (300, 'DistantHub', 'Hub', 'L', ?)")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool).await.unwrap();
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (100, 128672167, 3200, 0, 8, 0, ?, 'test')")
        .bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (300, 128672167, 0, 21500, 0, 500, ?, 'test')")
        .bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT OR REPLACE INTO user_state (user_id, current_system, cargo_capacity, jump_range_ly, pad_size_max, updated_at) VALUES ('test', 'Rare1', 500, 30.0, 'L', ?)")
        .bind(&now).execute(&pool).await.unwrap();

    elite_trade_finder_lib::ingest::ingestor::rebuild_latest_market(&conn).await.unwrap();

    let routes = rare_chains::find(&conn, "test", &ScoreWeights::default(), 10, None)
        .await
        .unwrap();
    assert!(!routes.is_empty(), "expected at least one rare chain");
}
