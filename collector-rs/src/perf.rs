//! Game-server perf endpoint: the small JSON document a server exposes over HTTP
//! describing its current vitals. Every field except `tps`/`mspt`/`memory_*`
//! defaults, so a server that hasn't been updated to emit CPU/player counts yet
//! still deserializes cleanly (the missing gauges read as 0).

use std::time::Duration;

use serde::Deserialize;

/// One sample from a game server's perf HTTP endpoint (e.g. the Minestom
/// `PerfHttpServer` at `GET http://<host>:<port+1000>/`).
#[derive(Debug, Clone, Deserialize)]
pub struct GamePerf {
    pub tps: f64,
    pub mspt: f64,
    /// Process CPU load 0.0–1.0 (0 if the server doesn't report it).
    #[serde(default)]
    pub cpu_process: f64,
    /// System CPU load 0.0–1.0 (0 if the server doesn't report it).
    #[serde(default)]
    pub cpu_system: f64,
    /// Heap bytes in use.
    pub memory_used: i64,
    #[serde(default)]
    pub memory_total: i64,
    pub memory_max: i64,
    #[serde(default)]
    pub players: u32,
    #[serde(default)]
    pub max_players: u32,
    #[serde(default)]
    pub spark_enabled: bool,
    #[serde(default)]
    pub last_spark_url: Option<String>,
    #[serde(default)]
    pub last_spark_url_at: i64,
}

/// GET `<base_url>/` and deserialize the perf JSON. `base_url` is the perf root
/// (e.g. `http://localhost:26566`); a trailing slash is added if missing. Uses a
/// 2-second timeout so a hung/slow server never stalls the collector loop.
pub async fn fetch_game_perf(client: &reqwest::Client, base_url: &str) -> anyhow::Result<GamePerf> {
    let url = format!("{}/", base_url.trim_end_matches('/'));
    let perf = client
        .get(&url)
        .timeout(Duration::from_secs(2))
        .send()
        .await?
        .error_for_status()?
        .json::<GamePerf>()
        .await?;
    Ok(perf)
}
