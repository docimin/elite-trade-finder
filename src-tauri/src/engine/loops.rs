use super::scoring::{compute_score, cr_per_hour, estimate_cycle_seconds, ScoreInputs};
use super::single_hop;
use crate::db::Db;
use crate::types::{RankedRoute, RouteLeg, RouteMode, ScoreWeights, Sustainability};
use anyhow::Result;
use std::collections::HashMap;

pub async fn find_two_leg(
    db: &Db,
    user_id: &str,
    weights: &ScoreWeights,
    limit: i32,
    override_ship: Option<&crate::types::ShipSpec>,
) -> Result<Vec<RankedRoute>> {
    // Fetch a wide pool of single-hops. Loops need BOTH A→B and B→A present,
    // so small pools miss most reverse pairs. latest_market makes this cheap.
    let hops = single_hop::find(db, user_id, weights, 5000, override_ship).await?;
    let mut by_pair: HashMap<(String, String), Vec<RankedRoute>> = HashMap::new();
    for hop in hops {
        let leg = &hop.legs[0];
        let k = (leg.from_station.clone(), leg.to_station.clone());
        by_pair.entry(k).or_default().push(hop);
    }

    let mut loops = Vec::<RankedRoute>::new();
    let keys: Vec<_> = by_pair.keys().cloned().collect();
    let mut seen = std::collections::HashSet::new();
    for (a, b) in &keys {
        // Only process each unordered pair once
        let pair_key = if a < b {
            (a.clone(), b.clone())
        } else {
            (b.clone(), a.clone())
        };
        if !seen.insert(pair_key) {
            continue;
        }
        let reverse = (b.clone(), a.clone());
        let Some(ab_list) = by_pair.get(&(a.clone(), b.clone())) else {
            continue;
        };
        let Some(ba_list) = by_pair.get(&reverse) else {
            continue;
        };
        let ab = &ab_list[0];
        let ba = &ba_list[0];

        let profit = ab.profit_per_cycle + ba.profit_per_cycle;
        let total_jumps = ab.legs[0].jumps + ba.legs[0].jumps;
        let cycle_seconds =
            estimate_cycle_seconds(&[(ab.legs[0].jumps, 1000.0), (ba.legs[0].jumps, 1000.0)]);
        let cphr = cr_per_hour(profit, cycle_seconds);

        let age_min = ((ab.freshest_age_seconds + ba.freshest_age_seconds) as f64) / 120.0;
        let avg_profit_per_ton = (ab.legs[0].profit_per_ton + ba.legs[0].profit_per_ton) / 2;
        let units = (ab.legs[0].supply.min(ab.legs[0].demand))
            .min(ba.legs[0].supply.min(ba.legs[0].demand));
        let score = compute_score(
            &ScoreInputs {
                profit_per_ton: avg_profit_per_ton,
                cargo_units: units,
                age_minutes: age_min,
                traffic_percentile: 0.5,
                touches_fleet_carrier: ab.touches_fleet_carrier || ba.touches_fleet_carrier,
                trip_jumps: total_jumps,
            },
            weights,
        );

        let worst_demand = ab.legs[0].demand.min(ba.legs[0].demand);
        let sustainability = if worst_demand >= units.max(1) * 10 {
            Sustainability::Sustainable
        } else {
            Sustainability::Decaying {
                estimated_cycles: (worst_demand / units.max(1)).max(1),
            }
        };

        let route_hash = format!(
            "loop2:{}<->{}:{}:{}",
            a, b, ab.legs[0].commodity, ba.legs[0].commodity
        );

        loops.push(RankedRoute {
            mode: RouteMode::Loop2,
            legs: vec![ab.legs[0].clone(), ba.legs[0].clone()],
            cr_per_hour: cphr,
            profit_per_cycle: profit,
            cycle_seconds,
            total_jumps,
            sustainability,
            score,
            freshest_age_seconds: ab.freshest_age_seconds.min(ba.freshest_age_seconds),
            touches_fleet_carrier: ab.touches_fleet_carrier || ba.touches_fleet_carrier,
            route_hash,
        });
    }

    loops.sort_by(|a, b| b.cr_per_hour.cmp(&a.cr_per_hour));
    loops.truncate(limit.max(1) as usize);
    Ok(loops)
}

pub async fn find_multi_leg(
    db: &Db,
    user_id: &str,
    weights: &ScoreWeights,
    max_legs: i32,
    limit: i32,
    override_ship: Option<&crate::types::ShipSpec>,
) -> Result<Vec<RankedRoute>> {
    assert!(max_legs <= 4, "max_legs capped at 4 per spec");
    let hops = single_hop::find(db, user_id, weights, 5000, override_ship).await?;

    let mut adj: HashMap<String, Vec<RouteLeg>> = HashMap::new();
    for hop in &hops {
        adj.entry(hop.legs[0].from_station.clone())
            .or_default()
            .push(hop.legs[0].clone());
    }

    let user_station: Option<String> = match db {
        Db::Sqlite(p) => sqlx::query_scalar(
            "SELECT current_station FROM user_state WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(p)
        .await
        .ok()
        .flatten()
        .flatten(),
        Db::Postgres(p) => sqlx::query_scalar(
            "SELECT current_station FROM user_state WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(p)
        .await
        .ok()
        .flatten()
        .flatten(),
    };

    let starts: Vec<String> = match user_station {
        Some(s) if adj.contains_key(&s) => vec![s],
        _ => adj.keys().cloned().collect(),
    };

    fn dfs(
        node: &str,
        start: &str,
        depth: i32,
        max_depth: i32,
        adj: &HashMap<String, Vec<RouteLeg>>,
        path: &mut Vec<RouteLeg>,
        out: &mut Vec<Vec<RouteLeg>>,
    ) {
        if depth >= max_depth {
            return;
        }
        let Some(next) = adj.get(node) else {
            return;
        };
        for leg in next {
            if path.iter().any(|p| p.to_station == leg.to_station) {
                continue;
            }
            path.push(leg.clone());
            if leg.to_station == start && path.len() >= 3 {
                out.push(path.clone());
            } else if leg.to_station != *start {
                dfs(&leg.to_station, start, depth + 1, max_depth, adj, path, out);
            }
            path.pop();
        }
    }

    let mut results = Vec::<RankedRoute>::new();
    for start in &starts {
        let mut cycles: Vec<Vec<RouteLeg>> = vec![];
        let mut path = Vec::new();
        dfs(start, start, 0, max_legs, &adj, &mut path, &mut cycles);

        for cycle in cycles {
            let mode = match cycle.len() {
                3 => RouteMode::Loop3,
                4 => RouteMode::Loop4,
                _ => continue,
            };
            let profit: i64 = cycle
                .iter()
                .map(|l| (l.profit_per_ton as i64) * (l.supply.min(l.demand) as i64))
                .sum();
            let total_jumps: i32 = cycle.iter().map(|l| l.jumps).sum();
            let leg_times: Vec<(i32, f64)> = cycle.iter().map(|l| (l.jumps, 1000.0)).collect();
            let cycle_seconds = estimate_cycle_seconds(&leg_times);
            let cphr = cr_per_hour(profit, cycle_seconds);

            let avg_ppt =
                cycle.iter().map(|l| l.profit_per_ton).sum::<i32>() / (cycle.len() as i32);
            let min_units = cycle
                .iter()
                .map(|l| l.supply.min(l.demand))
                .min()
                .unwrap_or(0);
            let age = cycle
                .iter()
                .map(|l| (chrono::Utc::now() - l.recorded_at).num_seconds())
                .max()
                .unwrap_or(0);
            let score = compute_score(
                &ScoreInputs {
                    profit_per_ton: avg_ppt,
                    cargo_units: min_units,
                    age_minutes: age as f64 / 60.0,
                    traffic_percentile: 0.5,
                    touches_fleet_carrier: false,
                    trip_jumps: total_jumps,
                },
                weights,
            );

            let route_hash = format!(
                "loop{}:{}",
                cycle.len(),
                cycle
                    .iter()
                    .map(|l| format!("{}->{}:{}", l.from_station, l.to_station, l.commodity))
                    .collect::<Vec<_>>()
                    .join("|")
            );

            let worst_demand = cycle.iter().map(|l| l.demand).min().unwrap_or(0);
            let sustainability = if worst_demand >= min_units.max(1) * 10 {
                Sustainability::Sustainable
            } else {
                Sustainability::Decaying {
                    estimated_cycles: (worst_demand / min_units.max(1)).max(1),
                }
            };

            results.push(RankedRoute {
                mode,
                legs: cycle,
                cr_per_hour: cphr,
                profit_per_cycle: profit,
                cycle_seconds,
                total_jumps,
                sustainability,
                score,
                freshest_age_seconds: age as i32,
                touches_fleet_carrier: false,
                route_hash,
            });
        }
    }

    results.sort_by(|a, b| b.cr_per_hour.cmp(&a.cr_per_hour));
    results.truncate(limit.max(1) as usize);
    Ok(results)
}
