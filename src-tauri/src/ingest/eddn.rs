use crate::state::AppState;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use flate2::read::ZlibDecoder;
use serde::Deserialize;
use std::io::Read;
use std::time::Duration;
use tokio::sync::mpsc;
use zeromq::{Socket, SocketRecv, SubSocket};

pub enum Eddn {
    CommodityV3(CommodityMsg),
    Ignored,
}

#[derive(Debug, Clone)]
pub struct CommodityMsg {
    pub timestamp: DateTime<Utc>,
    pub system_name: String,
    pub station_name: String,
    pub market_id: i64,
    pub station_type: Option<String>,
    pub software_name: String,
    pub gateway_timestamp: DateTime<Utc>,
    pub commodities: Vec<CommodityRow>,
}

#[derive(Debug, Clone)]
pub struct CommodityRow {
    pub name: String,
    pub buy_price: i32,
    pub sell_price: i32,
    pub mean_price: i32,
    pub stock: i32,
    pub demand: i32,
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(rename = "$schemaRef")]
    schema_ref: String,
    #[serde(default)]
    header: Header,
    message: serde_json::Value,
}

#[derive(Deserialize, Default)]
#[allow(non_snake_case)]
struct Header {
    #[serde(default, rename = "uploaderID")]
    _uploader_id: String,
    #[serde(default, rename = "softwareName")]
    software_name: String,
    #[serde(default, rename = "softwareVersion")]
    _software_version: String,
    #[serde(rename = "gatewayTimestamp")]
    gateway_timestamp: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct RawCommodityMsg {
    timestamp: DateTime<Utc>,
    systemName: String,
    stationName: String,
    marketId: i64,
    #[serde(default)]
    stationType: Option<String>,
    #[serde(default)]
    commodities: Vec<RawRow>,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct RawRow {
    name: String,
    buyPrice: i32,
    sellPrice: i32,
    #[serde(default)]
    meanPrice: i32,
    #[serde(default)]
    stock: i32,
    #[serde(default)]
    demand: i32,
}

pub fn decompress(bytes: &[u8]) -> Result<String> {
    let mut d = ZlibDecoder::new(bytes);
    let mut s = String::new();
    d.read_to_string(&mut s)?;
    Ok(s)
}

pub fn decode_json(raw: &str) -> Result<Eddn> {
    let env: Envelope = serde_json::from_str(raw)?;
    if env.header.software_name == "elite-trade-finder" {
        return Ok(Eddn::Ignored);
    }
    if !env.schema_ref.contains("/commodity/3") {
        return Ok(Eddn::Ignored);
    }
    let msg: RawCommodityMsg = serde_json::from_value(env.message)?;
    let gw = env
        .header
        .gateway_timestamp
        .ok_or_else(|| anyhow!("missing gateway ts"))?;
    Ok(Eddn::CommodityV3(CommodityMsg {
        timestamp: msg.timestamp,
        system_name: msg.systemName,
        station_name: msg.stationName,
        market_id: msg.marketId,
        station_type: msg.stationType,
        software_name: env.header.software_name,
        gateway_timestamp: gw,
        commodities: msg
            .commodities
            .into_iter()
            .map(|r| CommodityRow {
                name: r.name,
                buy_price: r.buyPrice,
                sell_price: r.sellPrice,
                mean_price: r.meanPrice,
                stock: r.stock,
                demand: r.demand,
            })
            .collect(),
    }))
}

pub fn spawn(
    state: AppState,
    relay_url: String,
    out: mpsc::Sender<CommodityMsg>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut backoff = Duration::from_secs(1);
        loop {
            match run_once(&state, &relay_url, &out).await {
                Ok(()) => backoff = Duration::from_secs(1),
                Err(e) => {
                    tracing::warn!(error = %e, "eddn stream disconnected, backing off {:?}", backoff);
                    state.eddn_status.write().await.connected = false;
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(30));
                }
            }
        }
    })
}

async fn run_once(
    state: &AppState,
    relay_url: &str,
    out: &mpsc::Sender<CommodityMsg>,
) -> Result<()> {
    let mut socket = SubSocket::new();
    socket.connect(relay_url).await?;
    socket.subscribe("").await?;

    state.eddn_status.write().await.connected = true;
    tracing::info!(relay_url, "eddn subscriber connected");

    // Rolling 10-second window: cheap, smooth, and doesn't lie with 0 during
    // quiet seconds when the feed is otherwise healthy.
    const WINDOW_SECS: usize = 10;
    let commodity_counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let activity_counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

    // Metric ticker: every second, slide the window forward, publish rate.
    let state_for_ticker = state.clone();
    let cc_for_ticker = commodity_counter.clone();
    let ac_for_ticker = activity_counter.clone();
    let ticker = tokio::spawn(async move {
        use std::sync::atomic::Ordering;
        let mut commodity_window: std::collections::VecDeque<u32> =
            std::collections::VecDeque::from(vec![0; WINDOW_SECS]);
        let mut activity_window: std::collections::VecDeque<u32> =
            std::collections::VecDeque::from(vec![0; WINDOW_SECS]);
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.tick().await;
        loop {
            interval.tick().await;
            let commodity_this_sec = cc_for_ticker.swap(0, Ordering::SeqCst);
            let activity_this_sec = ac_for_ticker.swap(0, Ordering::SeqCst);
            commodity_window.push_back(commodity_this_sec);
            activity_window.push_back(activity_this_sec);
            if commodity_window.len() > WINDOW_SECS {
                commodity_window.pop_front();
            }
            if activity_window.len() > WINDOW_SECS {
                activity_window.pop_front();
            }
            let commodity_per_sec =
                commodity_window.iter().copied().sum::<u32>() as f64 / WINDOW_SECS as f64;
            let activity_per_sec =
                activity_window.iter().copied().sum::<u32>() as f64 / WINDOW_SECS as f64;
            let mut st = state_for_ticker.eddn_status.write().await;
            // Show the non-zero of the two: commodity rate is what matters for
            // route data, but during market-quiet moments we fall back to the
            // total activity rate so "0.0" doesn't scare the user.
            st.msgs_per_sec = if commodity_per_sec > 0.0 {
                commodity_per_sec
            } else {
                activity_per_sec
            };
        }
    });

    // Main receive loop: counts go into atomics; the ticker reads them.
    let result: Result<()> = async {
        use std::sync::atomic::Ordering;
        loop {
            let msg = socket.recv().await?;
            activity_counter.fetch_add(1, Ordering::SeqCst);
            let frame = match msg.iter().next() {
                Some(f) => f.to_vec(),
                None => continue,
            };
            let json = match decompress(&frame) {
                Ok(s) => s,
                Err(e) => {
                    tracing::debug!(error = %e, "zlib fail");
                    continue;
                }
            };
            match decode_json(&json) {
                Ok(Eddn::CommodityV3(m)) => {
                    commodity_counter.fetch_add(1, Ordering::SeqCst);
                    state.eddn_status.write().await.last_msg_at = Some(chrono::Utc::now());
                    let _ = out.send(m).await;
                }
                Ok(Eddn::Ignored) => {}
                Err(e) => tracing::debug!(error = %e, "decode fail"),
            }
        }
    }
    .await;

    ticker.abort();
    result
}
