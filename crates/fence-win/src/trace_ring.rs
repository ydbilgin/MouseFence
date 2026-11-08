//! One hook producer and one tray consumer. Fixed storage; no locks, allocations or I/O in push.
use fence_types::EdgeRecord;
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};
pub const CAPACITY: usize = 16_384;

#[derive(Default, Debug)]
struct Slot {
    features: [AtomicI32; 12],
    qpc: AtomicU64,
    time: AtomicU32,
    x: AtomicI32,
    y: AtomicI32,
    state: AtomicU32,
}

#[derive(Debug)]
pub struct TraceRing {
    slots: Box<[Slot]>,
    head: AtomicU64,
    tail: AtomicU64,
    pub dropped: AtomicU64,
}

impl Default for TraceRing {
    fn default() -> Self {
        Self {
            slots: (0..CAPACITY).map(|_| Slot::default()).collect(),
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
        }
    }
}

impl TraceRing {
    /// Only the hook thread calls this. Full rings drop the new record without touching unread data.
    pub fn push(&self, record: EdgeRecord) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= CAPACITY as u64 {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        let slot = &self.slots[(head % CAPACITY as u64) as usize];
        for (field, value) in slot.features.iter().zip(record.features) {
            field.store(value, Ordering::Relaxed);
        }
        slot.qpc.store(record.qpc_us, Ordering::Relaxed);
        slot.time.store(record.time_ms, Ordering::Relaxed);
        slot.x.store(record.x, Ordering::Relaxed);
        slot.y.store(record.y, Ordering::Relaxed);
        slot.state.store(
            u32::from(record.action) | (u32::from(record.model_state) << 8),
            Ordering::Relaxed,
        );
        self.head.store(head.wrapping_add(1), Ordering::Release);
        true
    }
    /// Only the tray thread calls this; any Vec growth is on that thread.
    pub fn drain(&self, out: &mut Vec<EdgeRecord>) {
        let mut tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        while tail != head {
            let slot = &self.slots[(tail % CAPACITY as u64) as usize];
            let state = slot.state.load(Ordering::Relaxed);
            out.push(EdgeRecord {
                features: slot.features.each_ref().map(|n| n.load(Ordering::Relaxed)),
                qpc_us: slot.qpc.load(Ordering::Relaxed),
                time_ms: slot.time.load(Ordering::Relaxed),
                x: slot.x.load(Ordering::Relaxed),
                y: slot.y.load(Ordering::Relaxed),
                action: state as u8,
                model_state: (state >> 8) as u8,
            });
            tail = tail.wrapping_add(1);
        }
        self.tail.store(tail, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn j24_ring_is_bounded_and_wraps_without_overwriting_unread_records() {
        let ring = TraceRing::default();
        let mut out = Vec::with_capacity(CAPACITY);
        for cycle in 0..3 {
            for i in 0..CAPACITY {
                let x = (cycle * CAPACITY + i) as i32;
                assert!(ring.push(EdgeRecord {
                    features: [0; 12],
                    x,
                    y: -x,
                    qpc_us: x as u64,
                    time_ms: x as u32,
                    action: 2,
                    model_state: 31
                }));
            }
            assert!(!ring.push(EdgeRecord::default()));
            out.clear();
            ring.drain(&mut out);
            assert_eq!(out.len(), CAPACITY);
            for (i, r) in out.iter().enumerate() {
                let x = (cycle * CAPACITY + i) as i32;
                assert_eq!(
                    (r.x, r.y, r.qpc_us, r.time_ms, r.action, r.model_state),
                    (x, -x, x as u64, x as u32, 2, 31)
                );
            }
        }
        assert_eq!(ring.dropped.load(Ordering::Relaxed), 3);
    }
    #[test]
    fn j24_spsc_publishes_complete_reports_in_order() {
        let ring = std::sync::Arc::new(TraceRing::default());
        let producer = ring.clone();
        let join = std::thread::spawn(move || {
            for x in 0..100_000 {
                let r = EdgeRecord {
                    features: [0; 12],
                    x,
                    y: -x,
                    qpc_us: x as u64,
                    time_ms: x as u32,
                    action: (x % 3) as u8,
                    model_state: (x % 255) as u8,
                };
                while !producer.push(r) {
                    std::thread::yield_now();
                }
            }
        });
        let mut received = 0;
        let mut out = Vec::with_capacity(CAPACITY);
        while received < 100_000 {
            out.clear();
            ring.drain(&mut out);
            for r in &out {
                assert_eq!(
                    (r.x, r.y, r.qpc_us, r.time_ms, r.action, r.model_state),
                    (
                        received,
                        -received,
                        received as u64,
                        received as u32,
                        (received % 3) as u8,
                        (received % 255) as u8
                    )
                );
                received += 1;
            }
            std::thread::yield_now();
        }
        join.join().unwrap();
    }
}
