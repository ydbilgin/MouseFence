//! Callback timing: initialize off the hook thread, then only atomic reads and QPC queries.
use std::sync::atomic::{AtomicU64, Ordering};
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

static FREQUENCY: AtomicU64 = AtomicU64::new(0);

pub fn initialize() {
    let mut frequency = 0i64;
    // SAFETY: plain read-only counter query and valid out-pointer.
    if unsafe { QueryPerformanceFrequency(&mut frequency) } != 0 && frequency > 0 {
        FREQUENCY.store(frequency as u64, Ordering::Release);
    }
}

pub fn now_us() -> u64 {
    let frequency = FREQUENCY.load(Ordering::Acquire);
    if frequency == 0 {
        return 0;
    }
    let mut ticks = 0i64;
    // SAFETY: plain read-only counter query and valid out-pointer.
    if unsafe { QueryPerformanceCounter(&mut ticks) } == 0 || ticks < 0 {
        return 0;
    }
    (ticks as u128 * 1_000_000 / frequency as u128).min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    #[test]
    fn f10_qpc_frequency_is_cached_before_callbacks_and_time_is_monotonic() {
        super::initialize();
        let first = super::now_us();
        let second = super::now_us();
        assert!(first > 0 && second >= first);
    }
}
