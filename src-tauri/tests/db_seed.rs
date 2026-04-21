use elite_trade_finder_lib::db;
use tempfile::TempDir;

#[tokio::test]
async fn seed_inserts_commodities() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();
    let n = db::seed::commodities(&conn).await.unwrap();
    assert!(n >= 7, "expected >= 7 commodities seeded, got {n}");

    let pool = match &conn {
        db::Db::Sqlite(p) => p.clone(),
        _ => panic!(),
    };
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM commodities")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(count >= 7);
}

#[tokio::test]
async fn seed_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();
    db::seed::commodities(&conn).await.unwrap();
    db::seed::commodities(&conn).await.unwrap();

    let pool = match &conn {
        db::Db::Sqlite(p) => p.clone(),
        _ => panic!(),
    };
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM commodities")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 7);
}
