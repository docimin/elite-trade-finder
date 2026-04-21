use elite_trade_finder_lib::ingest::eddn;

#[test]
fn decodes_commodity_v3() {
    let raw = std::fs::read_to_string("tests/fixtures/eddn/commodity_v3.json").unwrap();
    let msg = eddn::decode_json(&raw).expect("decode");
    match msg {
        eddn::Eddn::CommodityV3(m) => {
            assert_eq!(m.system_name, "CD-37 6398");
            assert_eq!(m.station_name, "Hopper Point");
            assert_eq!(m.market_id, 3710000001);
            assert_eq!(m.commodities.len(), 2);
            let onion = m
                .commodities
                .iter()
                .find(|c| c.name == "OnionheadGammaStrain")
                .unwrap();
            assert_eq!(onion.sell_price, 21500);
            assert_eq!(onion.buy_price, 3200);
            assert_eq!(onion.stock, 450);
        }
        _ => panic!("expected CommodityV3"),
    }
}

#[test]
fn rejects_other_schemas_as_ignored() {
    let unknown = r#"{"$schemaRef":"https://eddn.edcd.io/schemas/shipyard/2","header":{"uploaderID":"x","softwareName":"x","softwareVersion":"x","gatewayTimestamp":"2026-04-20T12:00:00Z"},"message":{}}"#;
    assert!(matches!(
        eddn::decode_json(unknown),
        Ok(eddn::Eddn::Ignored)
    ));
}

#[test]
fn decompress_zlib_roundtrip() {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;
    let raw = std::fs::read_to_string("tests/fixtures/eddn/commodity_v3.json").unwrap();
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(raw.as_bytes()).unwrap();
    let compressed = enc.finish().unwrap();
    let out = eddn::decompress(&compressed).unwrap();
    assert_eq!(out, raw);
}
