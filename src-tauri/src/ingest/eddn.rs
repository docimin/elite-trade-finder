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

    let mut msgs_last_sec: u32 = 0;
    let mut last_metric = tokio::time::Instant::now();

    loop {
        let msg = socket.recv().await?;
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
                msgs_last_sec += 1;
                let _ = out.send(m).await;
            }
            Ok(Eddn::Ignored) => {}
            Err(e) => tracing::debug!(error = %e, "decode fail"),
        }
        if last_metric.elapsed() >= Duration::from_secs(1) {
            let mut st = state.eddn_status.write().await;
            st.msgs_per_sec = msgs_last_sec as f64;
            st.last_msg_at = Some(chrono::Utc::now());
            msgs_last_sec = 0;
            last_metric = tokio::time::Instant::now();
        }
    }
}
