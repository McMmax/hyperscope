# Hyperscope Native Profiling Contract and Provider-Neutral Foundations

**Status:** Frozen Contract (Phase 16 - `hyperscope3.contract`)  
**Date:** 2026-09-07  
**Coordinator Plan:** `monoio-full-cutover-2026-09-06`  
**Task ID:** `hyperscope3.contract`  
**Owner Paths:** `libs/hyperscope/docs`, `agent-coord/reports/hyperscope3/contract`  

---

## 1. Scope and Architectural Purpose

Hyperscope serves as the unified native performance console and telemetry foundation for Stomzy V3. In earlier JVM-based implementations, profiling relied heavily on the Spark profiler embedded inside the Java process. In Stomzy V3 (the pure-Rust Monoio/io_uring architecture), Spark is deprecated and replaced by modern, native, provider-neutral telemetry and profiling systems:

1. **Grafana Pyroscope**: High-frequency, continuous native CPU sample profiling and historical flame graphs.
2. **Prometheus / Grafana**: Bounded time-series metrics scraped from an external collector, displayed via Grafana dashboards.
3. **Tracy**: Detailed, low-overhead microsecond timeline captures, system zones, lock waits, and tick markers.
4. **SpacetimeDB 2.8.1**: Bounded live telemetry bus, real-time control queues, and job orchestration.
5. **Deprecated Spark Stub**: Legacy Spark contracts preserved for historical data compatibility and future out-of-process JVM/Paper/Spigot adapters. Zero Spark jars, zero JVM runtimes, and zero JNI are brought into the native V3 binary.

This specification establishes the authoritative, frozen contract for provider-neutral profiling, instrumented scope ownership, measurement procedures, and frontend integrations before downstream implementation begins.

---

## 2. Deployed Runtime and Library Version Pins

To guarantee deterministic behavior, prevent runtime conflicts, and ensure symbol resolution on deployment hosts, the following versions and targets are strictly pinned:

### 2.1 Host and Toolchain Pins

| Target / Tool | Pinned Version / Value | Architecture / Target Triple | Purpose / Constraints |
| :--- | :--- | :--- | :--- |
| **Operating System** | Linux 5.15+ (Debian 12 / Ubuntu 22.04 LTS) | `x86_64-unknown-linux-gnu` | Production deployment host with `io_uring` support |
| **Rust Toolchain** | `rustc 1.85.0+` (stable), 2021 edition | Host native / cross-target | Standardized workspace edition; `#![forbid(unsafe_code)]` in workspace packages |
| **Async I/O Engine** | `monoio 0.2.4` (`monoio::IoUringDriver`) | Pure Rust | Dataplane event loop; **STRICT EXCLUSION: Zero in-process Tokio, zero JVM** |
| **SpacetimeDB Host** | SpacetimeDB 2.8.1 | Linux x86_64 standalone | Live state bus, control commands, and server vitals |
| **SpacetimeDB WASM** | `spacetimedb 2.8.1` SDK | `wasm32-unknown-unknown` | Host module database reducers and schema |

### 2.2 Telemetry and Profiling Library Pins

| Library / Tool | Pinned Version | Features / Cargo Specification | Role in Architecture |
| :--- | :--- | :--- | :--- |
| **`prometheus`** | `0.13.4` | `default-features = false` | Standard metric collector for counters, gauges, and histograms |
| **`metrics`** | `0.24.1` | Facade crate | Runtime-independent metric facade for module decoupling |
| **`pyroscope-rs`** | `0.8.0` | `pyroscope`, `pyroscope_dummy` | Native Rust client bindings for continuous CPU profiling |
| **`pyroscope-ebpf`** | `0.8.0` | Linux eBPF unwinding | External eBPF CPU collector on Linux deployment host |
| **`tracy-client`** | `0.17.5` | `enable`, `manual-lifetime`, `fibers` | Microsecond timeline profiler; protocol version v0.10/v0.11 |
| **`tracing`** | `0.1.41` | `attributes`, `std` | Structured logging and span facade |
| **`tracing-core`** | `0.1.33` | Core span abstraction | Low-level dispatch without runtime dependencies |
| **`flecs_ecs`** | `0.1.20` | `flecs` C-core, stats addon enabled | Pinned ECS with native system timing and entity metrics |
| **`reqwest`** | `0.12.9` | `rustls-tls`, `json` | Used strictly in external collectors/sidecars; **NEVER in V3 binary** |

### 2.3 Binary Isolation Invariant

- **V3 Native Binary Closure:** Must contain **zero** references to `tokio`, `spark`, `minestom`, `jvm.dll`, `libjvm.so`, or Java classloaders.
- **External Collector Closure:** Daemons and sidecars (e.g., `cruisersd`, Prometheus scrapers, Pyroscope agents) may run their own runtimes (such as Tokio or external Go daemons) out-of-process.

---

## 3. Supported Native Profiler Capability Matrix

Hyperscope generalizes profile capture into provider-neutral jobs. Profiler providers expose explicit capabilities. Requests for unsupported capabilities fail closed with typed error codes rather than silently downgrading or attempting invalid operations.

| Feature / Capability | Pyroscope Provider | Tracy Provider | Deprecated Spark Stub |
| :--- | :--- | :--- | :--- |
| **Provider ID** | `pyroscope` | `tracy` | `spark` |
| **Target Runtime** | `native` (Rust / C Linux) | `native` (Rust / C Linux) | `jvm` (Paper / Spigot future adapter) |
| **Execution State in V3** | **Active Native Default** | **Active Native On-Demand** | **Deprecated Stub (Non-executable)** |
| **Sampling Mechanism** | Statistical timer / eBPF (100 Hz default) | Manual instrumentation + zone hooks | Thread stack sampler ($10\mu\text{s}$ historically) |
| **CPU Flame Graphs** | **Supported** (Process-wide native) | Unsupported (timeline only) | Unsupported in native V3 (typed stub) |
| **Timeline / Tick Capture**| Unsupported | **Supported** (Tick/system zones, locks) | Unsupported |
| **Allocation Profiling** | **Unsupported** in eBPF CPU collector | **Unsupported** (no alloc hook in V3) | Unsupported in native V3 |
| **Off-CPU / Wait Profiling**| Unsupported | **Supported** (Zone lock waits) | Unsupported in native V3 |
| **Historical Query API** | **Supported** via Pyroscope HTTP API | Unsupported (stream/file capture only) | **Supported for historical records only** |
| **Symbol Resolution** | DWARF / Build ID / ELF symbols | In-process symbol table / Tracy viewer | JVM method signatures |
| **Live Control Method** | SpacetimeDB `hyperscope_command` | SpacetimeDB `hyperscope_command` | Returns `ERR_PROVIDER_DEPRECATED_FOR_V3` |
| **Artifact Output** | Flame graph JSON / pprof / SVG | Native Tracy trace (`.tracy` file) | Historical URL / typed rejection |

### 3.1 Spark Deprecation and Future JVM Boundary

1. In native Stomzy V3, any command requesting a Spark profile (`kind = "profile_start"`, `provider = "spark"`) is intercepted by the profiling executor.
2. The executor immediately responds with status `"error"` and result JSON:
   ```json
   {
     "error_code": "ERR_PROVIDER_DEPRECATED_FOR_V3",
     "message": "Spark profiler is deprecated for Stomzy native V3. Use Pyroscope for CPU profiling or Tracy for timeline capture.",
     "future_adapter": "paper-spigot-jvm",
     "suggested_provider": "pyroscope"
   }
   ```
3. Under no circumstances will a native node download Spark JARs, spawn a JVM subprocess, or fail over silently.
4. Historical `spark_profile` table rows remain readable by the frontend and compatibility adapters to allow analysis of past runs.
5. Future Paper/Spigot platforms will implement the provider contract in an out-of-process JVM service without altering native V3 contracts.

---

## 4. Measured Overhead Baseline Procedure and Numeric Budgets

Hyperscope enforces strict numeric overhead budgets to prevent performance regression in production. Simulated historical benchmarks (such as the legacy "0.85 ms" claim) are explicitly rejected as valid baselines. All baselines must be measured using monotonic hardware clocks under identical, reproducible workloads.

### 4.1 Measurement Protocol

Measurements are conducted across four explicit operating regimes under identical workloads (1,000 simulated PLAY clients at 20.0 TPS):

1. **Regime 0 (`baseline_disabled`):** Telemetry completely disabled (`hyperscope_settings.enabled = false`). All metric updates bypassed or no-op; zero network scrapers; zero background samplers.
2. **Regime 1 (`basic_metrics`):** Telemetry enabled (`enabled = true`). In-memory static metric counters and bounded histograms active; Prometheus external scraper active at 2.0-second interval; zero CPU profilers.
3. **Regime 2 (`cpu_sampling`):** Basic metrics active + continuous Pyroscope CPU sampling at 100 Hz (10 ms sampling period) using external Linux eBPF / perf collector.
4. **Regime 3 (`detailed_tracy`):** Basic metrics active + Tracy client connected with system zones and frame markers, capturing a bounded 30-second trace.

### 4.2 Frozen Numeric Overhead Budgets

| Metric / Parameter | Baseline 1 (`basic_metrics`) | Baseline 2 (`cpu_sampling`) | Baseline 3 (`detailed_tracy`) | Rejection Threshold (Fail-Closed) |
| :--- | :--- | :--- | :--- | :--- |
| **Max Process CPU Overhead** | $\le 0.5\%$ of 1 core | $\le 1.5\%$ of process | $\le 3.0\%$ of process | $> 3.5\%$ process CPU |
| **Max Memory Overhead (RSS)**| $\le 4.0\text{ MB}$ RSS | $\le 16.0\text{ MB}$ RSS | $\le 64.0\text{ MB}$ bounded buffer | $> 70.0\text{ MB}$ buffer or leak |
| **p95 Tick Duration Inflation**| $\le 0.02\text{ ms}$ ($\le 20\mu\text{s}$) | $\le 0.05\text{ ms}$ ($\le 50\mu\text{s}$) | $\le 0.15\text{ ms}$ ($\le 150\mu\text{s}$) | $> 0.25\text{ ms}$ inflation |
| **p99 Tick Duration Inflation**| $\le 0.05\text{ ms}$ ($\le 50\mu\text{s}$) | $\le 0.15\text{ ms}$ ($\le 150\mu\text{s}$) | $\le 0.40\text{ ms}$ ($\le 400\mu\text{s}$) | $> 0.60\text{ ms}$ inflation |
| **Minimum Server TPS** | $20.0\text{ TPS}$ | $20.0\text{ TPS}$ | $20.0\text{ TPS}$ (0 missed ticks) | $< 20.0\text{ TPS}$ |
| **Capture Duration Ceiling** | N/A (continuous) | Continuous / Bounded | **Strict 30 seconds maximum** | $> 30\text{ seconds}$ without cancel |
| **Scraper Response Latency** | $\le 15\text{ ms}$ | $\le 25\text{ ms}$ | N/A | $> 50\text{ ms}$ |

### 4.3 Clock Source and Sampling Invariant

- Timing instrumentation MUST use a monotonic clock (`CLOCK_MONOTONIC` via `std::time::Instant`).
- Under no circumstances may system wall-clock timestamps (`CLOCK_REALTIME` / `SystemTime`) be subtracted to calculate execution duration.
- Histograms must resolve microsecond-scale operations without heap allocation on the hot path.

---

## 5. Finite Metric Labels and Cardinality Protection

Unbounded metric labels destroy time-series database stability, cause memory exhaustion, and crash Prometheus scrapers. Hyperscope mandates strict label cardinality bounds.

### 5.1 Permitted Finite Label Registry

Every Prometheus and Hyperscope metric emitted from native V3 code must use ONLY the following bounded label dimensions:

| Label Dimension | Permitted Finite Value Set | Max Cardinality |
| :--- | :--- | :--- |
| `module` | `runtime`, `transport`, `authority`, `ecs`, `world`, `gameplay`, `content`, `spacetimedb`, `hyperscope` | 9 |
| `stage` | `read`, `decode`, `frame`, `compress`, `encode`, `write`, `context_assembly`, `geometry_query`, `prediction`, `verdict_apply`, `system_exec`, `diff_compute`, `flush`, `reducer_call` | 14 |
| `world` | Pre-configured world identifiers: `main`, `lobby`, `nether`, `end` | 4 |
| `result` | `ok`, `error`, `dropped`, `timeout` | 4 |
| `profiler` | `pyroscope`, `tracy`, `spark` | 3 |
| `component` | `stomzy`, `internode`, `cruiserd` | 3 |

**Total theoretical cross-product ceiling per metric:** $\le 300$ active series across the entire server cluster.

### 5.2 Strictly Prohibited Unbounded Labels

The following fields are **strictly prohibited** from metric labels:
- **NO** Player UUIDs, account names, or usernames.
- **NO** IP addresses, socket addresses, or hostnames.
- **NO** Session IDs, connection IDs, or channel identifiers.
- **NO** Raw packet IDs, opcode numbers, or sequence numbers.
- **NO** Entity IDs, entity UUIDs, or Flecs entity IDs.
- **NO** Tick numbers, timestamps, or monotonic counters.
- **NO** Coordinate tuples (`x, y, z`) or chunk coordinates (`cx, cz`).
- **NO** Arbitrary user input, error messages, or exception strings.

### 5.3 Microsecond Histogram Bucket Standard

Histograms use 12 explicit, non-linear buckets optimized for microsecond-scale server tick work:
```
[10µs, 25µs, 50µs, 100µs, 250µs, 500µs, 1ms, 2.5ms, 5ms, 10ms, 25ms, 50ms]
```
Aggregation rule: Consumers MUST sum bucket counts across series; averaging p95 or p99 values across modules or servers is statistically invalid and prohibited.

---

## 6. CPU vs Elapsed Accounting and Async Safety

Misleading profiling attribution occurs when elapsed wall-clock time is confused with actual on-CPU execution time, or when parallel/nested tasks are summed naively. Hyperscope enforces rigorous accounting rules.

### 6.1 Core Accounting Principles

1. **Tick Wall-Clock Interval vs. Tick Execution Time:**
   - The Minecraft tick interval is nominally $50.0\text{ ms}$ ($20.0\text{ TPS}$).
   - **Tick Work Duration** is the elapsed duration spent actively executing systems, packets, and world simulation within that window.
   - A server running at $20.0\text{ TPS}$ with $0.8\text{ ms}$ tick execution time spends $49.2\text{ ms}$ sleeping or waiting for I/O. Hyperscope dashboards must present both values distinctly.
2. **Parallel and Nested Span Non-Additivity:**
   - Inner stages (e.g. `geometry_query` inside `context_assembly`) represent nested subsets of work. Their durations cannot be added together to compute a "total" that exceeds the parent span.
   - Multi-threaded or rayon/worker tasks executed concurrently on separate cores must report `cpu_time` and `elapsed_time` separately. Summed parallel time must NEVER be displayed as single-thread tick overrun.
3. **Statistical CPU Samples vs. Deterministic Timers:**
   - Pyroscope samples stack traces periodically (e.g., 100 times/second). Each sample represents on-CPU presence at sample time, not a precise elapsed stopwatch duration.
   - Dashboards must label Pyroscope metrics as "CPU Samples" or "CPU Percentage", never as exact elapsed microseconds.
4. **Memory Accounting Fidelity:**
   - Native Rust memory must report:
     - `rss_bytes`: Resident Set Size from operating system (`/proc/self/statm`).
     - `allocator_allocated_bytes`: Memory allocated via jemalloc/system allocator stats.
     - `cgroup_limit_bytes`: Container memory ceiling where present.
   - Telemetry must **never** relabel native RSS as "JVM Heap" or invent garbage collection (GC) pause gauges.

### 6.2 Rust Async Span Safety Contract

Monoio and Rust async runtimes multiplex multiple tasks on worker threads.
- **CRITICAL INVARIANT:** A thread-local entered span guard (e.g. `tracing::Span::enter()`) **MUST NEVER** be held across an `.await` boundary.
- Holding an entered guard across an `.await` causes subsequent polled tasks (which may belong to completely different modules) to be erroneously attributed to the earlier span.
- Async spans must use `.instrument(span)` on the Future, or explicitly drop the guard before awaiting.

---

## 7. Independent Failure Behavior and Fault Isolation

Hyperscope telemetry and profiling are non-critical observability tools. Under no circumstances may an observability failure impact the core game loop.

### 7.1 Fault Isolation Rules

1. **Fail-Open Game Loop:**
   - If Prometheus is unreachable, the Prometheus scraper times out, Grafana is down, Pyroscope collector terminates, or SpacetimeDB drops connection, **the game server must continue executing game ticks normally at 20.0 TPS**.
   - Metric updates, span creation, and profile hooks must be non-blocking.
2. **Fail-Closed Bounded Buffers:**
   - In-memory event rings and telemetry queues are bounded (maximum ring depth: 1,024 entries; maximum queue bytes: 4 MB).
   - If a collector or subscriber fails to consume events, the ring buffer drops entries and increments an internal counter (`hyperscope_dropped_samples_total`).
   - Unbounded memory queues, unbounded retries, or thread-blocking backpressure into the game engine are strictly prohibited.
3. **Explicit UI Telemetry States:**
   - Hyperscope UI panels must display explicit operational states:
     - `DISABLED`: Server telemetry explicitly turned off via settings.
     - `OFFLINE` / `UNAVAILABLE`: Server process reachable, but collector/agent offline.
     - `STALE`: Telemetry last reported $> 5\text{ seconds}$ ago (freshness indicator turns amber/red).
     - `SAMPLING_DROPPED`: Buffer overflow occurred; sample loss reported.
     - `CAPTURING`: Tracy or Pyroscope capture in progress with countdown timer.
     - `FAILED`: Profiler command failed with typed error code.
     - `COMPLETED`: Profile captured and ready for inspection.
   - **Missing data MUST be displayed as `--` or `N/A`**, NEVER as numeric `0` (which would misleadingly suggest 0% CPU or 0ms latency).

---

## 8. Frontend File Inventory and Integration Mapping

Hyperscope's frontend interface is hosted in the `dashvue` application. All frontend files interacting with Hyperscope, SpacetimeDB, and profiling have been inventoried and mapped:

| Frontend File Path | Role in Hyperscope Architecture | Current Dependencies & Subscriptions | Planned V3 Enhancements |
| :--- | :--- | :--- | :--- |
| `dashvue/app/components/shared/HyperscopePanel.vue` | Primary telemetry & profiler console panel | Subscribes to `server_vitals` & `hyperscope_settings`; calls `setHyperscopeEnabled`; posts to `/servers/{id}/spark/cmd` | Add provider selector (`pyroscope`, `tracy`), module drilldown table, Pyroscope flame graph iframe, and Tracy 30s capture trigger |
| `dashvue/app/components/shared/SparkFrame.vue` | Call-tree frame renderer | Hierarchical tree viewer for profile call frames | Retain for historical Spark profile inspection; adapt for generic hierarchical span summaries |
| `dashvue/app/pages/servers/[id]/index.vue` | Server details page | Embeds `HyperscopePanel` and Grafana iframe at line 1258 | Support deep-linking to correlated Pyroscope flame graphs and module breakdowns |
| `dashvue/app/pages/servers/index.vue` | Cluster server directory | Lists server cards with module badges (line 168: `{ key: 'hyperscope', label: 'HYPERSCOPE' }`) | Surface cluster-wide vitals health status and profiling indicators |
| `dashvue/app/components/forms/SpawnBackendModal.vue` | Server instantiation modal | Line 24: `ALL_MODULES = [..., 'hyperscope']` | Enables Hyperscope telemetry module upon backend server launch |
| `dashvue/app/spacetime/bindings/*` | Generated STDB SDK bindings | `HyperscopeCommandRow`, `HyperscopeSettingsRow`, `ClaimHyperscopeCommandReducer`, etc. | Provider-neutral table models will be added without breaking existing bindings |

---

## 9. Module Owner Paths and Downstream Task Boundaries

To prevent file conflicts across concurrent sprint tasks, file ownership is strictly delineated across Phase 16 work packages:

```
hyperscope3.contract (Phase 16 Foundation)
  ├── libs/hyperscope/docs/
  └── agent-coord/reports/hyperscope3/contract/

hyperscope3.providers (Phase 16 Providers & Schema)
  ├── libs/hyperscope/schema/
  ├── libs/hyperscope/collector-rs/
  └── libs/hyperscope/docs/providers/

hyperscope3.instrumentation (Phase 16 Core Facade)
  ├── libs/hyperscope/instrumentation/
  └── agent-coord/reports/hyperscope3/instrumentation/

hyperscope3.metrics (Phase 16 Prometheus & Grafana)
  ├── libs/hyperscope/metrics/
  └── agent-coord/reports/hyperscope3/metrics/

hyperscope3.pyroscope (Phase 16 Continuous CPU Profiling)
  ├── libs/hyperscope/pyroscope/
  └── agent-coord/reports/hyperscope3/pyroscope/

hyperscope3.tracy (Phase 16 Timeline Capture)
  ├── libs/hyperscope/tracy/
  └── agent-coord/reports/hyperscope3/tracy/

hyperscope3.sa (Phase 16 Server Authority Instrumentation)
  ├── stomzy/rust/authority/
  └── agent-coord/reports/hyperscope3/sa/

hyperscope3.flecs (Phase 16 Flecs ECS Systems & Stats)
  ├── stomzy/rust/ecs/
  └── agent-coord/reports/hyperscope3/flecs/

hyperscope3.spacetimedb (Phase 16 SpacetimeDB Telemetry)
  ├── libs/hyperscope/spacetimedb/
  └── agent-coord/reports/hyperscope3/spacetimedb/

hyperscope3.ui-modules (Phase 16 Scoreboard & Tablist)
  ├── stomzy/rust/modules/
  └── agent-coord/reports/hyperscope3/ui-modules/

hyperscope3.console (Phase 16 Frontend Hyperscope Cockpit)
  ├── dashvue/app/components/shared/HyperscopePanel.vue
  ├── dashvue/app/components/shared/SparkFrame.vue
  └── agent-coord/reports/hyperscope3/console/

hyperscope3.verification (Phase 16 Release Verification)
  ├── agent-coord/verification/
  └── agent-coord/reports/hyperscope3/verification/
```

---

## 10. Optional Server Authority (SA) and Future JVM Boundary

### 10.1 Optional Server Authority (SA) Boundary
- Server Authority is an optional module in Stomzy V3. Worlds configured with `sa_enabled: false` (e.g. creative lobbies, hub servers, minigame staging areas) **MUST NOT** execute SA prediction checks, context assembly, or raycasting.
- Telemetry hooks for SA must be completely bypassed when SA is disabled. Zero SA metrics, zero SA time-series, and zero SA span overhead may be incurred in SA-disabled worlds.
- Verification in downstream task `hyperscope3.sa` must demonstrate that toggling SA off results in strict zero SA CPU and memory footprint.

### 10.2 Future JVM Provider Boundary
- Existing Java sources in `libs/hyperscope/integration` and `libs/hyperscope/spark-upstream` remain preserved on disk. They are not deleted or discarded.
- Native Stomzy V3 does not execute Java.
- Future Paper/Spigot adapters will connect to Hyperscope by implementing an out-of-process daemon or direct SpacetimeDB client that publishes to `server_vitals` and claims `hyperscope_command` rows using the provider-neutral schema.
- This boundary ensures that Stomzy V3 maintains an uncompromising pure-Rust architecture while allowing the platform to support Paper/Spigot servers in subsequent releases.

---

## 11. Conclusion and Downstream Guidance

Task `hyperscope3.contract` successfully freezes the architectural invariants, pinned libraries, profiler capability matrix, measured overhead protocols, finite label constraints, accounting rules, failure policies, frontend mapping, and module boundaries. Downstream workers for `hyperscope3.providers`, `hyperscope3.instrumentation`, and subsequent tasks must adhere strictly to this contract.
