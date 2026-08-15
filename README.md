# Hyperscope

Full-stack performance telemetry for game-server networks, built on
[SpacetimeDB](https://spacetimedb.com). Hyperscope gives an admin panel a single,
live "which layer is hot right now" view across every component of a stack — game
servers, proxies, daemons — under one schema, plus a control plane for on-demand
profiling.

SpacetimeDB is the **live bus**: components report bounded vitals into a handful
of public tables, and the panel subscribes to them. Deep history and full profiler
call-trees live off-bus, so subscription sync stays cheap.

## What's in this repo

| Path | What it is | Language |
| --- | --- | --- |
| [`schema/`](schema) | `hyperscope-schema` — the `#[table]` contract (the source of truth an adopting SpacetimeDB module re-exports). | Rust |
| [`collector-rs/`](collector-rs) | `hyperscope-collector` — fetch a server's perf endpoint and report it via `report_game_vitals`. | Rust |
| [`panel-ts/`](panel-ts) | Shared Vue/TS helpers for rendering live vitals in the admin panel. | TypeScript |
| [`integration/`](integration) | JVM platform glue for the spark profiler (vendored Minestom adapter), consumed by Stomzy as the `:hyperscope` Gradle subproject. | Java |
| [`SPEC.md`](SPEC.md) | The contract: tables, reducer signatures, the perf-JSON → vitals mapping, and integration rules. | — |

The **reducers** (vitals ingest + profiling control plane) are intentionally **not**
in this repo — SpacetimeDB requires reducers to be compiled into the module crate,
and they typically carry host-specific concerns (audit logging, auth). The library
stays pure schema + collection + presentation so it can be shared verbatim.

## How a stack adopts Hyperscope

1. **Schema** — your SpacetimeDB module depends on `hyperscope-schema` and re-exports
   its tables so the macros register them into your module's WASM binary:
   ```rust
   // <your-module>/src/tables/hyperscope.rs
   pub use hyperscope_schema::{
       server_vitals, hyperscope_settings, spark_profile, hyperscope_command,
       ServerVitals, HyperscopeSettings, SparkProfile, HyperscopeCommand,
   };
   ```
   Add the matching reducers (`report_game_vitals`, `set_hyperscope_enabled`, …) to
   your module — see [`SPEC.md`](SPEC.md) for the exact signatures.

2. **Collector** — a daemon (or the server itself) polls each server's perf endpoint
   and reports it:
   ```rust
   let perf = hyperscope_collector::fetch_game_perf(&client, "http://localhost:26566").await?;
   let args = hyperscope_collector::GameVitalsArgs::from_game_perf("lobby", &perf);
   client.post(format!("{uri}/v1/database/{db}/call/report_game_vitals"))
       .json(&args.to_call_array()).send().await?;
   ```

3. **Panel** — subscribe to `server_vitals` + `hyperscope_settings` and render. The
   `panel-ts` helpers cover the health-grid colour math and the subscription queries.

## Status

`v0.1` ships **live vitals** end-to-end (CPU, TPS/MSPT, heap, players) plus the
per-server enable toggle. The profiling control plane (`spark_profile` /
`hyperscope_command`) tables ship in the schema and are wired for a follow-up.

## License

MIT © mearz — see [LICENSE](LICENSE). Exception: [`integration/`](integration)
contains vendored **GPL-3.0** spark source and carries its own
[`integration/LICENSE`](integration/LICENSE).
