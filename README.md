# 🔭 Hyperscope

**Hyperscope** is our full-stack performance telemetry and profiling suite, built natively on [SpacetimeDB](https://spacetimedb.com). 

---

## 🎯 What does it do?
Hyperscope gives developers and admins a live, real-time view of "which layer is hot right now" across the entire network. From Rust-based edge proxies down to the deepest Minestom game loops, Hyperscope tracks CPU time, memory allocations, and network latency in one unified dashboard.

## 🏗️ Architecture

- **`spark-upstream`**: Our tailored fork of the popular Spark profiler, optimized and tuned for our specific JVM footprint.
- **`collector-rs` (Rust)**: High-performance telemetry collector that runs on the edge, grabbing metric samples with negligible overhead and streaming them to the database.
- **`integration/` (Java)**: The bridge that links our Minestom servers with the metric pipelines, seamlessly combining Java profiling with Rust telemetry.

## 🚀 Key Features
- **Zero-Friction Profiling**: Automatically profiles at scale without severely degrading player experience.
- **SpacetimeDB Powered**: All telemetry data is pumped into SpacetimeDB, allowing for live SQL-like queries against currently running servers.
- **Full-Stack Vision**: Cross-references network latency from the edge proxies against tick duration from the Minestom nodes.
