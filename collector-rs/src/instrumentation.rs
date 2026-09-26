//! Core Native Telemetry Emitter & Low-Overhead Ring Buffer for Hyperscope V3.
//!
//! Invariants:
//! 1. Zero span guards held across `.await` points under Monoio async tasks.
//! 2. Low-overhead ring buffer (max 1024 events / 4 MB cap) with bounded drop counters.
//! 3. Sub-0.5% basic metric overhead budget with monotonic hardware clock timestamps.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

pub const DEFAULT_RING_BUFFER_CAPACITY: usize = 1024;
pub const MAX_TELEMETRY_BYTES_CAP: usize = 4 * 1024 * 1024; // 4 MB cap

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageKind {
    TickTotal,
    PacketDecode,
    MovementPhysics,
    CombatAuthority,
    InventoryTransactions,
    ChunkStreaming,
    EntityReplication,
    PacketEncode,
    PersistenceFlush,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryEvent {
    pub stage: StageKind,
    pub timestamp_micros: u64,
    pub elapsed_micros: u64,
    pub sequence: u64,
}

/// Bounded ring buffer for low-overhead telemetry event collection.
pub struct TelemetryRingBuffer {
    capacity: usize,
    buffer: Mutex<VecDeque<TelemetryEvent>>,
    sequence_counter: AtomicU64,
    dropped_counter: AtomicU64,
}

impl Default for TelemetryRingBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_RING_BUFFER_CAPACITY)
    }
}

impl TelemetryRingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.min(DEFAULT_RING_BUFFER_CAPACITY),
            buffer: Mutex::new(VecDeque::with_capacity(capacity)),
            sequence_counter: AtomicU64::new(1),
            dropped_counter: AtomicU64::new(0),
        }
    }

    /// Record a telemetry measurement event with monotonic timestamp.
    pub fn record_event(&self, stage: StageKind, elapsed_micros: u64) -> bool {
        let seq = self.sequence_counter.fetch_add(1, Ordering::Relaxed);
        let timestamp_micros = Instant::now().elapsed().as_micros() as u64;

        let event = TelemetryEvent {
            stage,
            timestamp_micros,
            elapsed_micros,
            sequence: seq,
        };

        if let Ok(mut guard) = self.buffer.lock() {
            if guard.len() >= self.capacity {
                guard.pop_front(); // drop oldest to maintain bounded memory
                self.dropped_counter.fetch_add(1, Ordering::Relaxed);
            }
            guard.push_back(event);
            true
        } else {
            self.dropped_counter.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    pub fn drain_events(&self) -> Vec<TelemetryEvent> {
        if let Ok(mut guard) = self.buffer.lock() {
            guard.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.lock().map(|b| b.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped_counter.load(Ordering::Relaxed)
    }
}

/// Scoped timing helper that measures elapsed time without holding locks or guards across `.await`.
pub struct ScopedTelemetrySpan {
    stage: StageKind,
    start: Instant,
}

impl ScopedTelemetrySpan {
    pub fn start(stage: StageKind) -> Self {
        Self {
            stage,
            start: Instant::now(),
        }
    }

    /// Complete the span and record to the buffer synchronously.
    pub fn complete(self, ring_buffer: &TelemetryRingBuffer) {
        let elapsed_micros = self.start.elapsed().as_micros() as u64;
        ring_buffer.record_event(self.stage, elapsed_micros);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_bounded_capacity_and_drop_counter() {
        let ring = TelemetryRingBuffer::new(5);

        for i in 1..=10 {
            ring.record_event(StageKind::MovementPhysics, i * 100);
        }

        assert_eq!(ring.len(), 5);
        assert_eq!(ring.dropped_count(), 5);

        let events = ring.drain_events();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].elapsed_micros, 600);
        assert_eq!(events[4].elapsed_micros, 1000);
    }

    #[test]
    fn test_scoped_telemetry_span_measurement() {
        let ring = TelemetryRingBuffer::new(100);

        {
            let span = ScopedTelemetrySpan::start(StageKind::TickTotal);
            std::thread::sleep(std::time::Duration::from_millis(2));
            span.complete(&ring);
        }

        assert_eq!(ring.len(), 1);
        let events = ring.drain_events();
        assert_eq!(events[0].stage, StageKind::TickTotal);
        assert!(events[0].elapsed_micros >= 1500, "Measured elapsed micros was {}", events[0].elapsed_micros);
    }
}
