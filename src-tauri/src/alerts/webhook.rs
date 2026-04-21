use crate::types::{RankedRoute, RouteMode};
use anyhow::Result;
use serde_json::json;

fn mode_label(m: &RouteMode) -> &'static str {
    match m {
        RouteMode::Single => "Single hop",
        RouteMode::Loop2 => "2-leg loop",
        RouteMode::Loop3 => "3-leg loop",
        RouteMode::Loop4 => "4-leg loop",
        RouteMode::RareChain => "Rare chain",
    }
}

pub async fn fire(url: &str, r: &RankedRoute) -> Result<()> {
    let cr_hr_m = r.cr_per_hour as f64 / 1_000_000.0;
    let path = {
        let mut parts: Vec<String> =
            r.legs.iter().map(|l| l.from_station.clone()).collect();
        if let Some(last) = r.legs.last() {
            parts.push(last.to_station.clone());
        }
        parts.join(" → ")
    };

    let commodities = r
        .legs
        .iter()
        .map(|l| format!("**{}** @ {} cr/t", l.commodity, l.profit_per_ton))
        .collect::<Vec<_>>()
        .join("\n");

    let distance_ly: f64 = r.legs.iter().map(|l| l.distance_ly).sum();
    let worst_ppt = r.legs.iter().map(|l| l.profit_per_ton).min().unwrap_or(0);

    let description = if commodities.is_empty() {
        path.clone()
    } else {
        format!("{}\n\n{}", path, commodities)
    };

    let payload = json!({
        "username": "Elite Trade Finder",
        "embeds": [{
            "title": format!("{:.1}M cr/hr · {}", cr_hr_m, mode_label(&r.mode)),
            "description": description,
            "color": 0xff7b00_u32,
            "fields": [
                {
                    "name": "Cycle profit",
                    "value": format!("{} cr", r.profit_per_cycle),
                    "inline": true,
                },
                {
                    "name": "Cycle time",
                    "value": format!("{} min", r.cycle_seconds / 60),
                    "inline": true,
                },
                {
                    "name": "Total jumps",
                    "value": format!("{}", r.total_jumps),
                    "inline": true,
                },
                {
                    "name": "Worst leg cr/t",
                    "value": format!("{}", worst_ppt),
                    "inline": true,
                },
                {
                    "name": "Total distance",
                    "value": format!("{:.1} ly", distance_ly),
                    "inline": true,
                },
                {
                    "name": "Fresh",
                    "value": format!("{}s ago", r.freshest_age_seconds),
                    "inline": true,
                },
            ],
            "footer": {
                "text": format!("route_hash: {}", r.route_hash)
            }
        }]
    });

    let res = reqwest::Client::new()
        .post(url)
        .json(&payload)
        .send()
        .await?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("webhook rejected ({}): {}", status, body);
    }
    Ok(())
}
