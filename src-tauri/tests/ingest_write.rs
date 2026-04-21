use chrono::Utc;
use elite_trade_finder_lib::db;
use elite_trade_finder_lib::ingest::eddn::{CommodityMsg, CommodityRow};
use elite_trade_finder_lib::ingest::ingestor;
use tempfile::TempDir;

fn sample_msg() -> CommodityMsg {
    CommodityMsg {
        timestamp: Utc::now(),
        system_name: "CD-37 6398".into(),
        station_name: "Hopper Point".into(),
        market_id: 3710000001,
        station_type: Some("Outpost".into()),
        software_name: "test".into(),
        gateway_timestamp: Utc::now(),
        commodities: vec![
            CommodityRow {
                name: "Gold".into(),
                buy_price: 9100,
                sell_price: 9500,
                mean_price: 9300,
                stock: 200,
                demand: 0,
            },
            CommodityRow {
                name: "OnionheadGammaStrain".into(),
                buy_price: 3200,
                sell_price: 21500,
                mean_price: 12000,
                stock: 450,
                demand: 0,
            },
        ],
    }
}

#[tokio::test]
async fn ingest_inserts_station_and_snapshots() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();
    db::seed::commodities(&conn).await.unwrap();

    let msg = sample_msg();
    let n = ingestor::ingest_commodity(&conn, &msg).await.unwrap();
    assert_eq!(n, 2);

    let pool = match &conn {
        db::Db::Sqlite(p) => p.clone(),
        _ => panic!(),
    };
    let (station_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM stations WHERE station_id = 3710000001")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(station_count, 1);

    let (snap_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM market_snapshots WHERE station_id = 3710000001")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(snap_count, 2);
}

#[tokio::test]
async fn ingest_is_idempotent_on_dedup() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();
    db::seed::commodities(&conn).await.unwrap();

    let msg = sample_msg();
    ingestor::ingest_commodity(&conn, &msg).await.unwrap();
    ingestor::ingest_commodity(&conn, &msg).await.unwrap();

    let pool = match &conn {
        db::Db::Sqlite(p) => p.clone(),
        _ => panic!(),
    };
    let (snap_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM market_snapshots")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(snap_count, 2);
}

#[tokio::test]
async fn unknown_commodity_is_auto_inserted_and_ingested() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();
    db::seed::commodities(&conn).await.unwrap();

    let mut msg = sample_msg();
    msg.commodities.push(CommodityRow {
        name: "MadeUpCommodity".into(),
        buy_price: 100,
        sell_price: 200,
        mean_price: 150,
        stock: 10,
        demand: 0,
    });
    let n = ingestor::ingest_commodity(&conn, &msg).await.unwrap();
    assert_eq!(n, 3);

    let pool = match &conn {
        db::Db::Sqlite(p) => p.clone(),
        _ => panic!(),
    };
    let (got_symbol,): (String,) = sqlx::query_as(
        "SELECT symbol FROM commodities WHERE symbol = 'MadeUpCommodity'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(got_symbol, "MadeUpCommodity");
}
