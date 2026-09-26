//! Scoreboard and Tablist UI Module Telemetry
//!
//! Provides granular measurement of render/diff time, changed lines/slots,
//! profile re-adds, and packet bytes with zero work done when disabled.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiModuleTelemetrySnapshot {
    pub module_name: String,
    pub render_count: u64,
    pub total_render_micros: u64,
    pub changed_elements_count: u64,
    pub unchanged_updates_avoided: u64,
    pub packets_emitted: u64,
    pub bytes_emitted: u64,
    pub enabled: bool,
}

#[derive(Debug)]
pub struct UiModuleTelemetry {
    module_name: String,
    enabled: AtomicBool,
    render_count: AtomicU64,
    total_render_micros: AtomicU64,
    changed_elements_count: AtomicU64,
    unchanged_updates_avoided: AtomicU64,
    packets_emitted: AtomicU64,
    bytes_emitted: AtomicU64,
}

impl UiModuleTelemetry {
    pub fn new(module_name: &str, enabled: bool) -> Arc<Self> {
        Arc::new(Self {
            module_name: module_name.to_string(),
            enabled: AtomicBool::new(enabled),
            render_count: AtomicU64::new(0),
            total_render_micros: AtomicU64::new(0),
            changed_elements_count: AtomicU64::new(0),
            unchanged_updates_avoided: AtomicU64::new(0),
            packets_emitted: AtomicU64::new(0),
            bytes_emitted: AtomicU64::new(0),
        })
    }

    #[inline(always)]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn record_render(
        &self,
        duration_micros: u64,
        changed: u64,
        avoided: u64,
        packets: u64,
        bytes: u64,
    ) {
        if !self.is_enabled() {
            return;
        }
        self.render_count.fetch_add(1, Ordering::Relaxed);
        self.total_render_micros.fetch_add(duration_micros, Ordering::Relaxed);
        self.changed_elements_count.fetch_add(changed, Ordering::Relaxed);
        self.unchanged_updates_avoided.fetch_add(avoided, Ordering::Relaxed);
        self.packets_emitted.fetch_add(packets, Ordering::Relaxed);
        self.bytes_emitted.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> UiModuleTelemetrySnapshot {
        UiModuleTelemetrySnapshot {
            module_name: self.module_name.clone(),
            render_count: self.render_count.load(Ordering::Relaxed),
            total_render_micros: self.total_render_micros.load(Ordering::Relaxed),
            changed_elements_count: self.changed_elements_count.load(Ordering::Relaxed),
            unchanged_updates_avoided: self.unchanged_updates_avoided.load(Ordering::Relaxed),
            packets_emitted: self.packets_emitted.load(Ordering::Relaxed),
            bytes_emitted: self.bytes_emitted.load(Ordering::Relaxed),
            enabled: self.is_enabled(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_module_scoreboard_and_tablist_diff_telemetry() {
        let scoreboard = UiModuleTelemetry::new("scoreboard", true);
        let tablist_disabled = UiModuleTelemetry::new("tablist", false);

        // Record scoreboard diff
        scoreboard.record_render(45, 3, 12, 3, 240);
        let snap = scoreboard.snapshot();
        assert_eq!(snap.render_count, 1);
        assert_eq!(snap.changed_elements_count, 3);
        assert_eq!(snap.unchanged_updates_avoided, 12);
        assert_eq!(snap.packets_emitted, 3);
        assert_eq!(snap.bytes_emitted, 240);

        // Disabled module does zero work
        tablist_disabled.record_render(50, 1, 10, 1, 80);
        let tab_snap = tablist_disabled.snapshot();
        assert_eq!(tab_snap.render_count, 0);
        assert_eq!(tab_snap.changed_elements_count, 0);
    }
}
