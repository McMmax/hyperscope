# Hyperscope contract

The wire contract every Hyperscope surface agrees on. The Rust `#[table]`
definitions in [`schema/src/lib.rs`](schema/src/lib.rs) are the source of truth;
this document is the human-readable companion (reducer signatures + mappings).

## Tables (`schema`)

All tables are `public` (panel-subscribable). Bounded by design — one row per
server for live state, pointers + top-N for profiles.

| Table | Key | Purpose |
| --- | --- | --- |
| `server_vitals` | `server_id` | One upserted row of live vitals per server. Superset of game + proxy gauges; `component` selects which are meaningful. Long tail rides in `extra_json`. |
| `hyperscope_settings` | `server_id` | Per-server master `enabled` toggle + sampler config. Absence/`false` ⇒ no telemetry. |
| `spark_profile` | `id` (auto) | Pointer + bounded top-N snapshot for one profiler run; raw blob lives off-bus at `blob_path`. |
| `hyperscope_command` | `id` (auto) | Control-plane queue — the panel enqueues, the owner claims/runs/acks. |

## Reducers (live in the **host module**, not this library)

SpacetimeDB compiles reducers into the module WASM, so adopters add these to their
own module. Argument order is load-bearing for HTTP `call` (positional JSON array).

**Ingest** (gated server-side by `hyperscope_settings.enabled`):
- `report_game_vitals(server_id: String, cpu_process: f64, cpu_system: f64, heap_used: i64, heap_max: i64, tps: f64, mspt: f64, players: u32, max_players: u32, extra_json: String)`
- `report_proxy_vitals(server_id, cpu_process, cpu_system, heap_used, heap_max, connections, conns_per_sec, bandwidth_in, bandwidth_out, backends_up, backends_total, route_latency_ms, extra_json)`

**Settings:** `set_hyperscope_enabled(server_id, enabled)`, `set_hyperscope_settings(server_id, enabled, vitals_interval_ms, background_profiler, sampler_interval_us)`

**Control plane:** `request_spark_profile(server_id, duration_secs, mode)`, `stop_spark_profile(server_id)`, `claim_hyperscope_command(id)`, `complete_hyperscope_command(id, status, result_json)`, `record_spark_profile(...)`, `update_spark_profile_status(id, status)`

## Game perf-JSON → `report_game_vitals` mapping (`collector-rs`)

A game server exposes a small JSON document over HTTP (the Minestom reference is
`PerfHttpServer` at `GET http://<host>:<minecraft_port + 1000>/`):

```json
{ "tps": 20.0, "mspt": 0.18, "cpu_process": 0.05, "cpu_system": 0.30,
  "memory_used": 199807936, "memory_total": 276824064, "memory_max": 6304038912,
  "players": 1, "max_players": 0, "spark_enabled": true,
  "last_spark_url": null, "last_spark_url_at": 0 }
```

| reducer arg | perf field |
| --- | --- |
| `cpu_process` / `cpu_system` | `cpu_process` / `cpu_system` (0.0–1.0) |
| `heap_used` / `heap_max` | `memory_used` / `memory_max` |
| `tps` / `mspt` | `tps` / `mspt` |
| `players` / `max_players` | `players` / `max_players` |
| `extra_json` | `{ memory_total, spark_enabled, last_spark_url, last_spark_url_at }` |

Every perf field except `tps`/`mspt`/`memory_*` is optional (serde-defaulted), so a
server that hasn't been updated to emit CPU/players still reports cleanly (those
gauges read 0).

## Integration rules

1. **Schema is shared; reducers + auth + audit are host-local.** The library never
   imports the host module. Adopters wrap the reducers with whatever audit/auth
   their module needs (the reference module logs to its own `audit_log`).
2. **Version lock-step.** `hyperscope-schema` pins `spacetimedb` to an exact version;
   it must match the host module's SpacetimeDB version (the `#[table]` macro emits
   version-specific descriptors).
3. **Collector posts unconditionally.** The `enabled` gate is enforced in the reducer,
   so collectors need not check settings before reporting.
