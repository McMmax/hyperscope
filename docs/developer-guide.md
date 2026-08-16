# 📊 Hyperscope: Microsecond CPU Profiler & Telemetry

> **Microsecond-Resolution Tick Profiler, Thread Sampler, and Spark-Compatible Telemetry Engine.**  
> *Hard-proof documentation for `libs/hyperscope`.*

---

## ⚡ 1. Overview & Architecture

`hyperscope` tracks execution duration of tick loops, packet decoders, and event dispatchers without introducing profiling overhead.

* **Thread Sampler**: Captures stack traces and CPU active time every $10\mu	ext{s}$.
* **Tick Telemetry**: Aggregates p50, p95, and p99 tick durations pushed to SpacetimeDB (`gamesense_perf`).
* **Spark Compatibility**: Exports profile snapshots in Spark viewer format for browser-based flamegraphs.

---

## 📖 Canonical Specifications
* **[`.bible/09-shared-libraries-and-apis`](file:///C:/Users/trapaholics/Documents/GitHub/.bible/09-shared-libraries-and-apis)**
