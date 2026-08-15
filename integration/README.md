# Hyperscope JVM integration

JVM-side glue for on-demand profiling of Minestom (Stomzy) game servers: the
Minestom platform adapter for [lucko's spark profiler](https://spark.lucko.me),
vendored from upstream spark source.

## What's here

- `src/main/java/me/lucko/spark/minestom/` — the spark Minestom platform glue
  (`SparkMinestom` builder + plugin/sender/tick-hook adapters), kept
  **byte-identical to upstream** (package and license headers unchanged) so it
  diffs cleanly against spark's source.
- `build.gradle.kts` — Gradle module definition. spark-common (the actual
  profiler engine) stays an **external** dependency from `repo.lucko.me`; only
  the thin platform adapter is vendored.

## How it's wired

This directory is not a Cargo workspace member — it is a Gradle subproject of
the **Stomzy** build (a sibling repo, `../Stomzy`):

```kotlin
// Stomzy/settings.gradle.kts
include(":hyperscope")
project(":hyperscope").projectDir = file("../Hyperscope/integration")
```

Consumers (`Stomzy/demo`, `games/*` backends) depend on it as
`implementation(project(":hyperscope"))` and import
`me.lucko.spark.minestom.SparkMinestom`. The Stomzy Dockerfile copies
`Hyperscope/integration` into the image alongside `Polar/integration` (the
Docker build context is the workspace root).

## License

The `me.lucko.spark.minestom` sources are vendored **GPL-3.0** spark code —
see [`LICENSE`](LICENSE) in this directory. The repository's root MIT license
does **not** cover this directory.

## Roadmap: `dev.terrorify.hyperscope`

Terrorify-owned metrics tooling lands in this same source set under
`dev.terrorify.hyperscope`. Entry point: `HyperscopeMinestom` — a facade
wrapping `SparkMinestom.builder(...)` that composes additional collectors
(GC/allocation, per-instance/chunk tick timings, network counters). Consumers
then swap `SparkMinestom.builder` for `HyperscopeMinestom.builder`, making the
GPL spark package an internal detail behind a Terrorify-owned API. The extra
metrics ride the `extra_json` field of `report_game_vitals` (see
[`../SPEC.md`](../SPEC.md)) via the engine's perf JSON endpoint, and the
`panel-ts` helpers render whatever lands there.
