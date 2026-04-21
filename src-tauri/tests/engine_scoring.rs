use elite_trade_finder_lib::engine::scoring::{compute_score, ScoreInputs};
use elite_trade_finder_lib::types::ScoreWeights;

fn base() -> ScoreInputs {
    ScoreInputs {
        profit_per_ton: 10_000,
        cargo_units: 100,
        age_minutes: 0.0,
        traffic_percentile: 0.5,
        touches_fleet_carrier: false,
        trip_jumps: 0,
    }
}

#[test]
fn base_profit_matters() {
    let big = compute_score(&base(), &ScoreWeights::default());
    let mut small = base();
    small.profit_per_ton = 1_000;
    assert!(big > compute_score(&small, &ScoreWeights::default()));
}

#[test]
fn freshness_decays_with_age() {
    let fresh = compute_score(&base(), &ScoreWeights::default());
    let mut old = base();
    old.age_minutes = 60.0;
    let old_s = compute_score(&old, &ScoreWeights::default());
    assert!(fresh > old_s, "fresh={fresh} old={old_s}");
}

#[test]
fn niche_bonus_favors_low_traffic() {
    let mut niche = base();
    niche.traffic_percentile = 0.1;
    let n = compute_score(&niche, &ScoreWeights::default());
    let mut popular = base();
    popular.traffic_percentile = 0.95;
    let p = compute_score(&popular, &ScoreWeights::default());
    assert!(n > p);
}

#[test]
fn fleet_carrier_bonus() {
    let without = compute_score(&base(), &ScoreWeights::default());
    let mut with_fc = base();
    with_fc.touches_fleet_carrier = true;
    let with = compute_score(&with_fc, &ScoreWeights::default());
    assert!(with > without);
}

#[test]
fn reachability_penalizes_long_trips() {
    let mut close = base();
    close.trip_jumps = 1;
    let c = compute_score(&close, &ScoreWeights::default());
    let mut far = base();
    far.trip_jumps = 20;
    let f = compute_score(&far, &ScoreWeights::default());
    assert!(c > f);
}

#[test]
fn weight_zero_disables_factor() {
    let mut base = base();
    base.age_minutes = 60.0;
    let mut weights = ScoreWeights::default();
    weights.freshness = 0.0;
    let no_freshness = compute_score(&base, &weights);
    let with_freshness = compute_score(&base, &ScoreWeights::default());
    assert!(
        no_freshness > with_freshness,
        "expected weight=0 to skip decay ({no_freshness} > {with_freshness})"
    );
}
