use chrono::Utc;
use elite_trade_finder_lib::ingest::journal::{self, JournalEvent};
use elite_trade_finder_lib::types::UserState;

fn blank_state() -> UserState {
    UserState {
        current_system: None,
        current_station: None,
        ship_type: None,
        cargo_capacity: None,
        jump_range_ly: None,
        credits: None,
        pad_size_max: None,
        updated_at: Utc::now(),
    }
}

#[test]
fn loadout_updates_ship_and_cargo() {
    let mut s = blank_state();
    journal::apply_event(
        &mut s,
        &JournalEvent::Loadout {
            ship: "type9".into(),
            cargo_capacity: 520,
            max_jump_range: 27.8,
        },
    );
    assert_eq!(s.ship_type.as_deref(), Some("type9"));
    assert_eq!(s.cargo_capacity, Some(520));
    assert_eq!(s.jump_range_ly, Some(27.8));
}

#[test]
fn fsd_jump_updates_system_and_clears_station() {
    let mut s = blank_state();
    s.current_station = Some("x".into());
    journal::apply_event(
        &mut s,
        &JournalEvent::FsdJump {
            star_system: "Sol".into(),
            star_pos: None,
        },
    );
    assert_eq!(s.current_system.as_deref(), Some("Sol"));
    assert!(s.current_station.is_none());
}

#[test]
fn docked_sets_station_but_not_pad() {
    let mut s = blank_state();
    journal::apply_event(
        &mut s,
        &JournalEvent::Docked {
            star_system: "Sol".into(),
            station_name: "Abraham Lincoln".into(),
            market_id: 1,
            station_type: Some("Orbis".into()),
            max_pad_size: Some("L".into()),
        },
    );
    assert_eq!(s.current_station.as_deref(), Some("Abraham Lincoln"));
    // Pad comes from the ship, not the station.
    assert!(s.pad_size_max.is_none());
}

#[test]
fn loadout_sets_pad_from_ship_type() {
    let mut s = blank_state();
    journal::apply_event(
        &mut s,
        &JournalEvent::Loadout {
            ship: "corsair".into(),
            cargo_capacity: 128,
            max_jump_range: 35.0,
        },
    );
    assert_eq!(s.pad_size_max.as_deref(), Some("M"));

    let mut s = blank_state();
    journal::apply_event(
        &mut s,
        &JournalEvent::Loadout {
            ship: "cutter".into(),
            cargo_capacity: 720,
            max_jump_range: 20.0,
        },
    );
    assert_eq!(s.pad_size_max.as_deref(), Some("L"));

    let mut s = blank_state();
    journal::apply_event(
        &mut s,
        &JournalEvent::Loadout {
            ship: "sidewinder".into(),
            cargo_capacity: 4,
            max_jump_range: 8.0,
        },
    );
    assert_eq!(s.pad_size_max.as_deref(), Some("S"));
}

#[test]
fn load_game_updates_credits() {
    let mut s = blank_state();
    journal::apply_event(
        &mut s,
        &JournalEvent::LoadGame { credits: 42 },
    );
    assert_eq!(s.credits, Some(42));
}
