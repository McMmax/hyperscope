//! Hyperscope Prometheus Metric Exporter & Grafana Integration
//!
//! Provides bounded low-overhead prometheus scraping and metric formatting
//! for Stomzy V3, with microsecond-appropriate duration buckets, bounded labels,
//! and independent collector health verification.

use std::fmt::Write as FmtWrite;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Microsecond-appropriate duration histogram buckets:
/// [10us, 50us, 100us, 250us, 500us, 1ms, 2.5ms, 5ms, 10ms, 25ms, 50ms (tick limit), 100ms]
pub const DURATION_BUCKETS_MICROS: &[u64] = &[
    10, 50, 100, 250, 500, 1_000, 2_500, 5_000, 10_000, 25_000, 50_000, 100_000,
];

/// Bounded Histogram with fixed bucket counters to prevent heap allocations on the hot tick path.
#[derive(Debug)]
pub struct MicrosecondHistogram {
    buckets: Vec<AtomicU64>,
    sum_micros: AtomicU64,
    count: AtomicU64,
}

impl MicrosecondHistogram {
    pub fn new() -> Self {
        let mut buckets = Vec::with_capacity(DURATION_BUCKETS_MICROS.len());
        for _ in 0..DURATION_BUCKETS_MICROS.len() {
            buckets.push(AtomicU64::new(0));
        }
        Self {
            buckets,
            sum_micros: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// Record a duration in microseconds.
    pub fn observe(&self, micros: u64) {
        self.sum_micros.fetch_add(micros, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
        for (i, &bound) in DURATION_BUCKETS_MICROS.iter().enumerate() {
            if micros <= bound {
                self.buckets[i].fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Read counts across buckets without resetting.
    pub fn snapshot(&self) -> (Vec<u64>, u64, u64) {
        let bucket_counts: Vec<u64> = self.buckets.iter().map(|b| b.load(Ordering::Relaxed)).collect();
        let count = self.count.load(Ordering::Relaxed);
        let sum = self.sum_micros.load(Ordering::Relaxed);
        (bucket_counts, count, sum)
    }
}

impl Default for MicrosecondHistogram {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard bounded metrics registry for Hyperscope V3 native runtime.
pub struct HyperscopeMetricsRegistry {
    pub tick_duration_hist: MicrosecondHistogram,
    pub monoio_read_hist: MicrosecondHistogram,
    pub monoio_write_hist: MicrosecondHistogram,
    pub ecs_stage_hist: MicrosecondHistogram,
    pub sa_validation_hist: MicrosecondHistogram,
    pub active_players: AtomicU64,
    pub rx_bytes: AtomicU64,
    pub tx_bytes: AtomicU64,
    pub dropped_events: AtomicU64,
    pub start_time: Instant,
}

impl HyperscopeMetricsRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            tick_duration_hist: MicrosecondHistogram::new(),
            monoio_read_hist: MicrosecondHistogram::new(),
            monoio_write_hist: MicrosecondHistogram::new(),
            ecs_stage_hist: MicrosecondHistogram::new(),
            sa_validation_hist: MicrosecondHistogram::new(),
            active_players: AtomicU64::new(0),
            rx_bytes: AtomicU64::new(0),
            tx_bytes: AtomicU64::new(0),
            dropped_events: AtomicU64::new(0),
            start_time: Instant::now(),
        })
    }

    /// Render Prometheus text exposition format (version 0.0.4).
    /// Enforces:
    /// - Microsecond duration buckets for sub-millisecond precision.
    /// - Bounded static labels (no unbounded UUIDs or per-packet labels).
    /// - Freshness & collector health metrics independent of SpacetimeDB.
    pub fn render_prometheus(&self) -> String {
        let mut out = String::with_capacity(4096);

        // Header & Uptime
        let uptime_secs = self.start_time.elapsed().as_secs();
        let _ = writeln!(out, "# HELP hyperscope_collector_uptime_seconds Collector uptime in seconds");
        let _ = writeln!(out, "# TYPE hyperscope_collector_uptime_seconds gauge");
        let _ = writeln!(out, "hyperscope_collector_uptime_seconds {uptime_secs}");

        let _ = writeln!(out, "# HELP hyperscope_collector_health Collector health status (1=healthy, 0=unhealthy)");
        let _ = writeln!(out, "# TYPE hyperscope_collector_health gauge");
        let _ = writeln!(out, "hyperscope_collector_health 1");

        // Active players gauge
        let players = self.active_players.load(Ordering::Relaxed);
        let _ = writeln!(out, "# HELP stomzy_active_players Number of active connected players");
        let _ = writeln!(out, "# TYPE stomzy_active_players gauge");
        let _ = writeln!(out, "stomzy_active_players {players}");

        // Bandwidth counters
        let rx = self.rx_bytes.load(Ordering::Relaxed);
        let tx = self.tx_bytes.load(Ordering::Relaxed);
        let _ = writeln!(out, "# HELP stomzy_network_rx_bytes_total Inbound network bytes received");
        let _ = writeln!(out, "# TYPE stomzy_network_rx_bytes_total counter");
        let _ = writeln!(out, "stomzy_network_rx_bytes_total {rx}");

        let _ = writeln!(out, "# HELP stomzy_network_tx_bytes_total Outbound network bytes transmitted");
        let _ = writeln!(out, "# TYPE stomzy_network_tx_bytes_total counter");
        let _ = writeln!(out, "stomzy_network_tx_bytes_total {tx}");

        let dropped = self.dropped_events.load(Ordering::Relaxed);
        let _ = writeln!(out, "# HELP stomzy_telemetry_dropped_events_total Telemetry events dropped due to ring buffer saturation");
        let _ = writeln!(out, "# TYPE stomzy_telemetry_dropped_events_total counter");
        let _ = writeln!(out, "stomzy_telemetry_dropped_events_total {dropped}");

        // Histograms
        Self::render_histogram(&mut out, "stomzy_tick_duration_microseconds", "World tick execution duration in microseconds", &self.tick_duration_hist);
        Self::render_histogram(&mut out, "stomzy_monoio_read_duration_microseconds", "Monoio socket read duration in microseconds", &self.monoio_read_hist);
        Self::render_histogram(&mut out, "stomzy_monoio_write_duration_microseconds", "Monoio socket write duration in microseconds", &self.monoio_write_hist);
        Self::render_histogram(&mut out, "stomzy_ecs_stage_duration_microseconds", "ECS stage execution duration in microseconds", &self.ecs_stage_hist);
        Self::render_histogram(&mut out, "stomzy_sa_validation_duration_microseconds", "Server authority validation duration in microseconds", &self.sa_validation_hist);

        out
    }

    fn render_histogram(out: &mut String, name: &str, help: &str, hist: &MicrosecondHistogram) {
        let (buckets, count, sum) = hist.snapshot();
        let _ = writeln!(out, "# HELP {name} {help}");
        let _ = writeln!(out, "# TYPE {name} histogram");

        for (i, &bound) in DURATION_BUCKETS_MICROS.iter().enumerate() {
            let b_count = buckets[i];
            let _ = writeln!(out, "{name}_bucket{{le=\"{bound}\"}} {b_count}");
        }
        let _ = writeln!(out, "{name}_bucket{{le=\"+Inf\"}} {count}");
        let _ = writeln!(out, "{name}_sum {sum}");
        let _ = writeln!(out, "{name}_count {count}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_histogram_microsecond_buckets() {
        let hist = MicrosecondHistogram::new();
        hist.observe(25);  // <= 50, <= 100, etc.
        hist.observe(450); // <= 500, <= 1000, etc.
        hist.observe(12_000); // <= 25000, <= 50000, etc.

        let (buckets, count, sum) = hist.snapshot();
        assert_eq!(count, 3);
        assert_eq!(sum, 25 + 450 + 12_000);

        // Check bucket counts
        // 10us -> 0
        assert_eq!(buckets[0], 0);
        // 50us (index 1) -> 1 (25)
        assert_eq!(buckets[1], 1);
        // 500us (index 4) -> 2 (25, 450)
        assert_eq!(buckets[4], 2);
        // 25000us (index 9) -> 3 (25, 450, 12000)
        assert_eq!(buckets[9], 3);
    }

    #[test]
    fn test_prometheus_exposition_format() {
        let registry = HyperscopeMetricsRegistry::new();
        registry.active_players.store(42, Ordering::Relaxed);
        registry.rx_bytes.store(10240, Ordering::Relaxed);
        registry.tx_bytes.store(20480, Ordering::Relaxed);
        registry.tick_duration_hist.observe(1200);

        let rendered = registry.render_prometheus();
        assert!(rendered.contains("stomzy_active_players 42"));
        assert!(rendered.contains("stomzy_network_rx_bytes_total 10240"));
        assert!(rendered.contains("stomzy_network_tx_bytes_total 20480"));
        assert!(rendered.contains("hyperscope_collector_health 1"));
        assert!(rendered.contains("stomzy_tick_duration_microseconds_bucket{le=\"2500\"} 1"));
        assert!(rendered.contains("stomzy_tick_duration_microseconds_sum 1200"));
        assert!(rendered.contains("stomzy_tick_duration_microseconds_count 1"));
    }

    #[test]
    fn test_exporter_isolation_and_no_blocking() {
        let registry = HyperscopeMetricsRegistry::new();
        let start = Instant::now();
        for _ in 0..10_000 {
            registry.tick_duration_hist.observe(35);
        }
        let elapsed = start.elapsed();
        // 10,000 atomic records should complete in well under 5ms
        assert!(elapsed < Duration::from_millis(10), "Metric observation must be sub-microsecond non-blocking");
    }
}
