//! Hyperscope — full-stack performance telemetry schema (SpacetimeDB).
//!
//! This crate is the **single source of truth** for the Hyperscope table
//! contract. An adopting SpacetimeDB module depends on it and re-exports these
//! `#[table]` structs from its own `tables` module so the macros register them
//! into the module's WASM binary:
//!
//! ```ignore
//! // <your-module>/src/tables/hyperscope.rs
//! pub use hyperscope_schema::{
//!     server_vitals, hyperscope_settings, spark_profile, hyperscope_command,
//!     ServerVitals, HyperscopeSettings, SparkProfile, HyperscopeCommand,
//! };
//! ```
//!
//! Reducers (the ingest + control-plane logic) are **not** part of this crate —
//! SpacetimeDB requires reducers to be compiled into the module crate, and they
//! typically carry host-specific concerns (audit logging, auth). This crate stays
//! pure schema so it can be shared verbatim across every component and project.
//!
//! Design notes:
//!   * SpacetimeDB is the **live bus**, not the metrics warehouse — these tables
//!     stay bounded (one upserted `server_vitals` row per server; profile pointers
//!     plus a bounded top-N snapshot). Deep history lives off-bus.
//!   * `hyperscope_settings.enabled` is the per-server master toggle; vitals are
//!     opt-in — a server reports nothing until enabled.
//!   * `hyperscope_command` is the control plane — the panel enqueues a command,
//!     the owning component/collector claims and runs it.

use spacetimedb::{table, Identity, Timestamp};

/// Latest live vitals for a server — exactly one upserted row per `server_id`.
/// Superset of game + proxy fields; component-irrelevant columns stay 0 and the
/// panel switches its display on `component`. The long tail (tick-phase
/// breakdown, per-route latency, entity/chunk counts, GC detail) rides in
/// `extra_json` so this row never needs a migration to carry a new gauge.
#[derive(Clone)]
#[table(accessor = server_vitals, public)]
pub struct ServerVitals {
    #[primary_key]
    pub server_id: String,
    /// "stomzy" | "internode" | … the reporting component kind.
    pub component: String,

    // ── universal process metrics ──────────────────────────────────────────
    /// Process CPU load, 0.0–1.0.
    pub cpu_process: f64,
    /// System-wide CPU load, 0.0–1.0.
    pub cpu_system: f64,
    /// Heap/RSS bytes in use.
    pub heap_used: i64,
    /// Heap/RSS ceiling in bytes.
    pub heap_max: i64,

    // ── game server ────────────────────────────────────────────────────────
    pub tps: f64,
    pub mspt: f64,
    pub players: u32,
    pub max_players: u32,

    // ── proxy ──────────────────────────────────────────────────────────────
    pub connections: u32,
    pub conns_per_sec: f64,
    pub bandwidth_in: i64,
    pub bandwidth_out: i64,
    pub backends_up: u32,
    pub backends_total: u32,
    /// Mean proxy→backend routing latency, milliseconds.
    pub route_latency_ms: f64,

    /// Component-specific breakdown as JSON (tick phases, per-route stats, …).
    pub extra_json: String,
    pub sampled_at: Timestamp,
}

/// Latest live vitals for a HOST — a physical/virtual machine running part of the
/// stack, or the SpacetimeDB container itself. One upserted row per `id`. Lets
/// the cockpit show whole-machine load (and the DB's own load) independently of
/// the per-server process metrics in `ServerVitals`.
#[derive(Clone)]
#[table(accessor = host_vitals, public)]
pub struct HostVitals {
    /// Stable id: a machine id/hostname, or "spacetimedb" for the DB container.
    #[primary_key]
    pub id: String,
    /// "machine" | "db".
    pub kind: String,
    /// Human label (hostname / "SpacetimeDB").
    pub label: String,
    /// Whole-host (or container) CPU load, 0.0–1.0.
    pub cpu: f64,
    /// Memory used / total, bytes.
    pub mem_used: i64,
    pub mem_total: i64,
    /// Disk used / total, bytes (0 when N/A, e.g. the DB container).
    pub disk_used: i64,
    pub disk_total: i64,
    /// Extra detail as JSON: machines carry `procs` (top processes by usage) +
    /// `net_in`/`net_out`/`pids`; the DB carries `net_in`/`net_out` (bandwidth),
    /// `io_read`/`io_write` (block I/O) + `pids`. Keeps the row migration-free.
    pub extra_json: String,
    pub sampled_at: Timestamp,
}

/// Per-server Hyperscope configuration. Absence (or `enabled = false`) means the
/// server reports no telemetry — the toggle the admin flips in the panel.
#[derive(Clone)]
#[table(accessor = hyperscope_settings, public)]
pub struct HyperscopeSettings {
    #[primary_key]
    pub server_id: String,
    /// Master toggle for this server's telemetry + profiling.
    pub enabled: bool,
    /// How often the component pushes live vitals, milliseconds.
    pub vitals_interval_ms: u32,
    /// Whether an always-on background profiler feeds Hyperscope.
    pub background_profiler: bool,
    /// Sampler interval, microseconds (e.g. 4000 = 4ms).
    pub sampler_interval_us: u32,
    pub updated_at: Timestamp,
    pub updated_by: Identity,
}

/// Pointer + bounded snapshot for one profiler run. The *full* call tree lives
/// off-bus (parsed from the raw blob at `blob_path`); STDB keeps only what the
/// cockpit needs to render "what's hot right now" without syncing megabytes.
#[derive(Clone)]
#[table(accessor = spark_profile, public,
        index(accessor = by_server, btree(columns = [server_id])))]
pub struct SparkProfile {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    // Indexed via the table-level `by_server` index above; a field-level
    // #[index(btree)] here would collide on the generated index name.
    pub server_id: String,
    pub component: String,
    /// Sampler engine: "async" | "java".
    pub engine: String,
    /// "execution" | "allocation".
    pub mode: String,
    /// "running" | "complete" | "failed".
    pub status: String,
    pub started_at: Timestamp,
    pub ended_at: Timestamp,
    pub duration_ms: u32,
    pub n_ticks: u32,
    pub interval_us: u32,
    /// Local path to the raw gzipped profile blob the collector kept.
    pub blob_path: String,
    /// Optional external viewer link (if also uploaded).
    pub viewer_url: String,
    /// Bounded top-N hottest frames as JSON, for the in-panel summary.
    pub top_frames_json: String,
    pub requested_by: Identity,
}

/// Control-plane queue. The panel enqueues a command (start/stop a profile,
/// flip background sampling); the owning component or collector claims it,
/// executes, and writes the result back — so every action is driveable from the
/// admin panel without a side HTTP channel.
#[derive(Clone)]
#[table(accessor = hyperscope_command, public,
        index(accessor = by_server_status, btree(columns = [server_id, status])))]
pub struct HyperscopeCommand {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub server_id: String,
    /// "profile_start" | "profile_stop" | "set_background".
    pub kind: String,
    /// Command arguments as JSON, e.g. `{"duration":30,"mode":"execution"}`.
    pub args_json: String,
    /// "pending" | "claimed" | "done" | "error".
    pub status: String,
    /// Executor's result payload (e.g. the new profile id) as JSON.
    pub result_json: String,
    pub created_at: Timestamp,
    pub created_by: Identity,
}
