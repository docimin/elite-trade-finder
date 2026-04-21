use crate::types::ScoreWeights;

#[derive(Debug, Clone)]
pub struct ScoreInputs {
    pub profit_per_ton: i32,
    pub cargo_units: i32,
    pub age_minutes: f64,
    pub traffic_percentile: f64,
    pub touches_fleet_carrier: bool,
    pub trip_jumps: i32,
}

pub fn compute_score(i: &ScoreInputs, w: &ScoreWeights) -> f64 {
    let base_profit = (i.profit_per_ton as f64) * (i.cargo_units as f64);

    let freshness_raw = (-i.age_minutes / 30.0).exp();
    let freshness = 1.0 + w.freshness * (freshness_raw - 1.0);

    let niche_raw = 1.0 + 0.5 * (1.0 - i.traffic_percentile);
    let niche = 1.0 + w.niche * (niche_raw - 1.0);

    let fc_raw = if i.touches_fleet_carrier { 1.3 } else { 1.0 };
    let fc = 1.0 + w.fleet_carrier * (fc_raw - 1.0);

    let reach_raw = 1.0 / (1.0 + (i.trip_jumps as f64) * 0.1);
    let reach = 1.0 + w.reachability * (reach_raw - 1.0);

    base_profit * freshness * niche * fc * reach
}

pub fn estimate_cycle_seconds(legs: &[(i32, f64)]) -> i32 {
    let mut total = 0.0;
    for (jumps, dist_ls) in legs {
        let jump_time = (*jumps as f64) * 50.0;
        let sc_time = (dist_ls / 1000.0).min(240.0).max(60.0);
        let dock_time = 60.0;
        let market_time = 15.0;
        total += jump_time + sc_time + dock_time + market_time;
    }
    total as i32
}

pub fn cr_per_hour(cycle_profit: i64, cycle_seconds: i32) -> i64 {
    if cycle_seconds <= 0 {
        return 0;
    }
    let hours = (cycle_seconds as f64) / 3600.0;
    ((cycle_profit as f64) / hours) as i64
}
