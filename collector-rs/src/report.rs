//! Mapping a [`GamePerf`] sample into the `report_game_vitals` reducer call.
//!
//! Reducer signature (positional):
//! ```ignore
//! report_game_vitals(
//!     server_id: String, cpu_process: f64, cpu_system: f64,
//!     heap_used: i64, heap_max: i64, tps: f64, mspt: f64,
//!     players: u32, max_players: u32, extra_json: String,
//! )
//! ```

use serde_json::{json, Value};

use crate::perf::GamePerf;

/// The arguments to the `report_game_vitals` reducer, in declaration order.
#[derive(Debug, Clone)]
pub struct GameVitalsArgs {
    pub server_id: String,
    pub cpu_process: f64,
    pub cpu_system: f64,
    pub heap_used: i64,
    pub heap_max: i64,
    pub tps: f64,
    pub mspt: f64,
    pub players: u32,
    pub max_players: u32,
    /// Component-specific extras carried opaquely in `server_vitals.extra_json`.
    pub extra_json: String,
}

impl GameVitalsArgs {
    /// Normalise a perf sample into reducer args. Memory ceiling/total, spark
    /// state, and the last profile URL ride in `extra_json` (the reducer's typed
    /// columns only cover the headline gauges).
    pub fn from_game_perf(server_id: impl Into<String>, perf: &GamePerf) -> Self {
        let extra = json!({
            "memory_total": perf.memory_total,
            "spark_enabled": perf.spark_enabled,
            "last_spark_url": perf.last_spark_url,
            "last_spark_url_at": perf.last_spark_url_at,
        });
        GameVitalsArgs {
            server_id: server_id.into(),
            cpu_process: perf.cpu_process,
            cpu_system: perf.cpu_system,
            heap_used: perf.memory_used,
            heap_max: perf.memory_max,
            tps: perf.tps,
            mspt: perf.mspt,
            players: perf.players,
            max_players: perf.max_players,
            extra_json: extra.to_string(),
        }
    }

    /// The positional JSON array POSTed as the reducer call body.
    pub fn to_call_array(&self) -> Value {
        json!([
            self.server_id,
            self.cpu_process,
            self.cpu_system,
            self.heap_used,
            self.heap_max,
            self.tps,
            self.mspt,
            self.players,
            self.max_players,
            self.extra_json,
        ])
    }
}
