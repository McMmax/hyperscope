//! Server Authority (SA) Stage Telemetry Instrumentation
//!
//! Provides granular measurement for Server Authority stages:
//! - Context assembly & prediction
//! - Geometry query & support check
//! - Raycast / AABB check duration & outcome
//! - Verdict application and receive-to-validation latency
//!
//! Invariants:
//! - SA-disabled worlds incur ZERO validation and ZERO measurement overhead.
//! - Preserves legal gameplay on SA-enabled worlds.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaCheckOutcome {
    Accept,
    RejectPhase,
    RejectReach,
    RejectDiscontinuity,
    GeometryMiss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaStageMetricsSnapshot {
    pub total_validations: u64,
    pub accept_count: u64,
    pub reject_count: u64,
    pub geometry_misses: u64,
    pub avg_validation_duration_micros: f64,
    pub avg_receive_to_validation_delay_micros: f64,
    pub sa_enabled: bool,
}

#[derive(Debug)]
pub struct ServerAuthorityTelemetry {
    enabled: AtomicBool,
    total_validations: AtomicU64,
    accept_count: AtomicU64,
    reject_count: AtomicU64,
    geometry_misses: AtomicU64,
    total_duration_micros: AtomicU64,
    total_delay_micros: AtomicU64,
}

impl ServerAuthorityTelemetry {
    pub fn new(enabled: bool) -> Arc<Self> {
        Arc::new(Self {
            enabled: AtomicBool::new(enabled),
            total_validations: AtomicU64::new(0),
            accept_count: AtomicU64::new(0),
            reject_count: AtomicU64::new(0),
            geometry_misses: AtomicU64::new(0),
            total_duration_micros: AtomicU64::new(0),
            total_delay_micros: AtomicU64::new(0),
        })
    }

    #[inline(always)]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Record a completed SA validation check.
    #[inline(always)]
    pub fn record_validation(
        &self,
        outcome: SaCheckOutcome,
        duration_micros: u64,
        receive_to_validation_delay_micros: u64,
    ) {
        if !self.is_enabled() {
            return;
        }

        self.total_validations.fetch_add(1, Ordering::Relaxed);
        self.total_duration_micros.fetch_add(duration_micros, Ordering::Relaxed);
        self.total_delay_micros.fetch_add(receive_to_validation_delay_micros, Ordering::Relaxed);

        match outcome {
            SaCheckOutcome::Accept => {
                self.accept_count.fetch_add(1, Ordering::Relaxed);
            }
            SaCheckOutcome::RejectPhase
            | SaCheckOutcome::RejectReach
            | SaCheckOutcome::RejectDiscontinuity => {
                self.reject_count.fetch_add(1, Ordering::Relaxed);
            }
            SaCheckOutcome::GeometryMiss => {
                self.geometry_misses.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Snapshot current SA telemetry metrics.
    pub fn snapshot(&self) -> SaStageMetricsSnapshot {
        let total = self.total_validations.load(Ordering::Relaxed);
        let accepts = self.accept_count.load(Ordering::Relaxed);
        let rejects = self.reject_count.load(Ordering::Relaxed);
        let misses = self.geometry_misses.load(Ordering::Relaxed);
        let duration_sum = self.total_duration_micros.load(Ordering::Relaxed);
        let delay_sum = self.total_delay_micros.load(Ordering::Relaxed);

        let avg_dur = if total > 0 { duration_sum as f64 / total as f64 } else { 0.0 };
        let avg_delay = if total > 0 { delay_sum as f64 / total as f64 } else { 0.0 };

        SaStageMetricsSnapshot {
            total_validations: total,
            accept_count: accepts,
            reject_count: rejects,
            geometry_misses: misses,
            avg_validation_duration_micros: avg_dur,
            avg_receive_to_validation_delay_micros: avg_delay,
            sa_enabled: self.is_enabled(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sa_disabled_incurs_zero_work() {
        let sa_telemetry = ServerAuthorityTelemetry::new(false);
        assert!(!sa_telemetry.is_enabled());

        sa_telemetry.record_validation(SaCheckOutcome::Accept, 15, 200);
        sa_telemetry.record_validation(SaCheckOutcome::RejectPhase, 20, 250);

        let snap = sa_telemetry.snapshot();
        assert_eq!(snap.total_validations, 0);
        assert_eq!(snap.accept_count, 0);
        assert_eq!(snap.reject_count, 0);
    }

    #[test]
    fn test_sa_enabled_records_stages_and_latency() {
        let sa_telemetry = ServerAuthorityTelemetry::new(true);
        assert!(sa_telemetry.is_enabled());

        sa_telemetry.record_validation(SaCheckOutcome::Accept, 10, 100);
        sa_telemetry.record_validation(SaCheckOutcome::RejectPhase, 20, 150);
        sa_telemetry.record_validation(SaCheckOutcome::GeometryMiss, 5, 50);

        let snap = sa_telemetry.snapshot();
        assert_eq!(snap.total_validations, 3);
        assert_eq!(snap.accept_count, 1);
        assert_eq!(snap.reject_count, 1);
        assert_eq!(snap.geometry_misses, 1);
        assert_eq!(snap.avg_validation_duration_micros, (10.0 + 20.0 + 5.0) / 3.0);
        assert_eq!(snap.avg_receive_to_validation_delay_micros, (100.0 + 150.0 + 50.0) / 3.0);
    }
}
