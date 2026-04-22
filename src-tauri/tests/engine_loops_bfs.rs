use chrono::Utc;
use elite_trade_finder_lib::db;
use elite_trade_finder_lib::engine::loops;
use elite_trade_finder_lib::types::{RouteMode, ScoreWeights};
use tempfile::TempDir;

#[tokio::test]
async fn finds_three_leg_cycle() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();
    db::seed::commodities(&conn).await.unwrap();

    let pool = match &conn {
        db::Db::Sqlite(p) => p.clone(),
        _ => panic!(),
    };
    sqlx::query("INSERT INTO systems (id64, name, x, y, z) VALUES (1,'A',0,0,0),(2,'B',5,0,0),(3,'C',5,5,0)")
        .execute(&pool).await.unwrap();
    for (id, sys, name) in [(100i64, "A", "SA"), (200, "B", "SB"), (300, "C", "SC")] {
        sqlx::query("INSERT INTO stations (station_id, system_name, station_name, pad_size, last_seen_at) VALUES (?, ?, ?, 'L', ?)")
            .bind(id).bind(sys).bind(name).bind(Utc::now().to_rfc3339())
            .execute(&pool).await.unwrap();
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (100, 128049152, 5000, 0, 500, 0, ?, 'test')").bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (200, 128049152, 0, 9500, 0, 5000, ?, 'test')").bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (200, 128049150, 3000, 0, 500, 0, ?, 'test')").bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (300, 128049150, 0, 7000, 0, 5000, ?, 'test')").bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (300, 128049205, 35000, 0, 500, 0, ?, 'test')").bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (100, 128049205, 0, 52000, 0, 5000, ?, 'test')").bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT OR REPLACE INTO user_state (user_id, current_system, current_station, cargo_capacity, jump_range_ly, pad_size_max, updated_at) VALUES ('test', 'A', 'SA', 500, 25.0, 'L', ?)")
        .bind(&now).execute(&pool).await.unwrap();

    elite_trade_finder_lib::ingest::ingestor::rebuild_latest_market(&conn).await.unwrap();

    let routes = loops::find_multi_leg(&conn, "test", &ScoreWeights::default(), 3, 5, None)
        .await
        .unwrap();
    let three_leg = routes.iter().find(|r| matches!(r.mode, RouteMode::Loop3));
    assert!(three_leg.is_some(), "expected a 3-leg loop to be found");
    let r = three_leg.unwrap();
    assert_eq!(r.legs.len(), 3);
    assert_eq!(r.legs[0].from_station, "SA");
    assert_eq!(r.legs[2].to_station, "SA");
}
