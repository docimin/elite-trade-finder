use elite_trade_finder_lib::db;
use tempfile::TempDir;

#[tokio::test]
async fn sqlite_file_url_connects() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    assert_eq!(conn.dialect(), "sqlite");
}

#[tokio::test]
async fn unknown_scheme_errors() {
    let result = db::connect("mysql://nope").await;
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("expected error"),
    };
    assert!(err.to_string().contains("unrecognized"));
}
