use elite_trade_finder_lib::ingest::journal;

#[test]
fn parses_loadout() {
    let lines = std::fs::read_to_string("tests/fixtures/journal/loadout.jsonl").unwrap();
    let events: Vec<_> = lines.lines().map(journal::parse_line).collect();
    let loadout = events
        .iter()
        .flatten()
        .find_map(|e| match e {
            journal::JournalEvent::Loadout {
                ship,
                cargo_capacity,
                max_jump_range,
                ..
            } => Some((ship.clone(), *cargo_capacity, *max_jump_range)),
            _ => None,
        })
        .expect("no Loadout event found");
    assert_eq!(loadout.0, "type9");
    assert_eq!(loadout.1, 520);
    assert!((loadout.2 - 27.8).abs() < 0.001);
}

#[test]
fn parses_fsd_jump() {
    let lines = std::fs::read_to_string("tests/fixtures/journal/loadout.jsonl").unwrap();
    let jump = lines
        .lines()
        .filter_map(journal::parse_line)
        .find_map(|e| match e {
            journal::JournalEvent::FsdJump { star_system, .. } => Some(star_system),
            _ => None,
        })
        .expect("no FSDJump");
    assert_eq!(jump, "LHS 3006");
}

#[test]
fn parses_docked_with_pad_inference() {
    let lines = std::fs::read_to_string("tests/fixtures/journal/docked.jsonl").unwrap();
    let docked = lines
        .lines()
        .filter_map(journal::parse_line)
        .find_map(|e| match e {
            journal::JournalEvent::Docked {
                station_name,
                max_pad_size,
                ..
            } => Some((station_name, max_pad_size)),
            _ => None,
        })
        .expect("no Docked");
    assert_eq!(docked.0, "Hopper Point");
    assert_eq!(docked.1.as_deref(), Some("M"));
}

#[test]
fn parses_market_json() {
    let blob = std::fs::read_to_string("tests/fixtures/journal/market.json").unwrap();
    let m = journal::parse_market_file(&blob).unwrap();
    assert_eq!(m.station_name, "Jameson Memorial");
    assert_eq!(m.items.len(), 2);
    let gold = m
        .items
        .iter()
        .find(|i| i.commodity_id == 128049152)
        .unwrap();
    assert_eq!(gold.buy_price, Some(9200));
    assert_eq!(gold.sell_price, Some(9500));
    let tritium = m
        .items
        .iter()
        .find(|i| i.commodity_id == 128049205)
        .unwrap();
    assert_eq!(tritium.buy_price, None);
    assert_eq!(tritium.sell_price, Some(52000));
}
