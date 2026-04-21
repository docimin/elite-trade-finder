use chrono::Utc;
use elite_trade_finder_lib::db;
use elite_trade_finder_lib::engine::loops;
use elite_trade_finder_lib::types::ScoreWeights;
use tempfile::TempDir;

#[tokio::test]
async fn finds_two_leg_loop() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();
    db::seed::commodities(&conn).await.unwrap();

    let pool = match &conn {
        db::Db::Sqlite(p) => p.clone(),
        _ => panic!(),
    };
    sqlx::query("INSERT INTO systems (id64, name, x, y, z) VALUES (1, 'Sol', 0,0,0), (2, 'LHS 3006', 5,2,1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO stations (station_id, system_name, station_name, pad_size, last_seen_at) VALUES (100, 'Sol', 'A', 'L', ?), (200, 'LHS 3006', 'B', 'L', ?)")
        .bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (100, 128049152, 5000, 0, 500, 0, ?, 'test')").bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (200, 128049152, 0, 9500, 0, 5000, ?, 'test')").bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (200, 128049205, 35000, 0, 500, 0, ?, 'test')").bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (100, 128049205, 0, 52000, 0, 5000, ?, 'test')").bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT OR REPLACE INTO user_state (user_id, current_system, cargo_capacity, jump_range_ly, pad_size_max, updated_at) VALUES ('test', 'Sol', 500, 25.0, 'L', ?)")
        .bind(&now).execute(&pool).await.unwrap();

    elite_trade_finder_lib::ingest::ingestor::rebuild_latest_market(&conn).await.unwrap();

    let routes = loops::find_two_leg(&conn, "test", &ScoreWeights::default(), 10)
        .await
        .unwrap();
    assert!(!routes.is_empty());
    let r = &routes[0];
    assert_eq!(r.legs.len(), 2);
    assert!(
        (r.legs[0].from_station == "A" && r.legs[0].to_station == "B"
            && r.legs[1].from_station == "B" && r.legs[1].to_station == "A")
            || (r.legs[0].from_station == "B" && r.legs[0].to_station == "A"
                && r.legs[1].from_station == "A" && r.legs[1].to_station == "B"),
        "legs should form A<->B"
    );
}
