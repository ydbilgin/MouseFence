//! C# `Platform/MouseGuard.cs` without the callback: what the tray thread knows about the hook.
//!
//! - `Enabled` is the intent (Start called, Stop not); `HookInstalled` is whether a handle is held. Windows
//!   can drop a hook silently, so the two differ (B024).
//! - Start resets the probe, and when no handle is held resets the engine and installs (B024).
//! - Stop resets the probe and unhooks.
//! - The probe (`check_health`) compares the callback counter and the cursor position with the previous
//!   probe; a Dead verdict starts a bounded recovery on a fresh hook thread (B010, B024). Failed installs
//!   also recover without cursor movement. Only callback traffic, or an explicit Start, re-arms retries.
//!
//! The hook itself (the step, the thread) is fence-win's; [`HookHost`] is the narrow port to it.

use fence_engine::hook_health;
use fence_types::{EngineConfig, HookHealthVerdict};

use crate::GuardPort;

// source: cx brief hook-recovery requires bounded retries/backoff; these timings/count are defaults.
const MAX_RECOVERY_ATTEMPTS: u8 = 3;
const RECOVERY_BACKOFF_MS: u64 = 3000;

/// The hook side as the tray thread drives it (production: fence-win `HookThread`, wired in apps/mousefence).
pub trait HookHost {
    fn monotonic_ms(&self) -> u64 {
        static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        EPOCH.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
    }
    fn recovery_log(&mut self, _message: &str) {}
    /// Reinstall on a fresh, desktop-ready thread in production. Fakes keep the ordered lifecycle.
    fn recover(&mut self) -> bool {
        if self.installed() {
            self.uninstall();
        }
        self.reset_engine();
        self.install()
    }
    fn metrics(&self) -> Option<fence_types::HookMetrics> {
        None
    }
    fn pause_trace(&mut self) {}
    fn configure_trace(
        &mut self,
        _settings: &fence_config::Settings,
        _monitors: &[fence_types::MonitorInfo],
        _state: fence_types::RuntimeState,
    ) {
    }
    fn drain_trace(&mut self, _state: fence_types::RuntimeState) -> usize {
        0
    }
    fn mark_accident(&mut self, _state: fence_types::RuntimeState) -> usize {
        0
    }
    /// Install the hook; true when a handle is held afterwards.
    fn install(&mut self) -> bool;
    fn uninstall(&mut self);
    /// A handle is held (C# `_hook != IntPtr.Zero`).
    fn installed(&self) -> bool;
    /// C# `_core.Reset()`, on the hook thread, in order with the moves.
    fn reset_engine(&mut self);
    /// C# `MouseGuard.Configure`: install a freshly compiled engine and reset (re-baseline).
    fn set_engine_config(&mut self, config: EngineConfig);
    /// Every callback since the hook thread started (C# `_callbacks`).
    fn callbacks(&self) -> u64;
    /// GetCursorPos (read-only); `None` when Windows refused.
    fn cursor_pos(&self) -> Option<(i32, i32)>;
    /// The live toggles the hook step reads (atomics; the hook never waits on the tray).
    fn set_toggles(&mut self, gate_open: bool, side_contain: bool, confine: bool);
    /// (contained panics, failed cursor placements) since the hook thread started (2.0, J08 and J07).
    fn fault_counts(&self) -> (u64, u64) {
        (0, 0)
    }
}

/// C# `MouseGuard` state kept on the tray thread.
#[derive(Debug)]
pub struct Guard<H: HookHost> {
    host: H,
    running: bool,
    callbacks_at_last_probe: u64,
    last_probe_point: Option<(i32, i32)>,
    gate_open: bool,
    side_contain: bool,
    confine: bool,
    recovery_attempts: u8,
    next_recovery_ms: u64,
    outage: bool,
}

impl<H: HookHost> Guard<H> {
    pub fn new(host: H) -> Self {
        Self {
            host,
            running: false,
            callbacks_at_last_probe: 0,
            last_probe_point: None,
            gate_open: false,
            side_contain: false,
            confine: false,
            recovery_attempts: 0,
            next_recovery_ms: 0,
            outage: false,
        }
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }

    pub fn gate_open(&self) -> bool {
        self.gate_open
    }

    pub fn side_contain(&self) -> bool {
        self.side_contain
    }

    pub fn confine(&self) -> bool {
        self.confine
    }

    fn push_toggles(&mut self) {
        self.host.set_toggles(self.gate_open, self.side_contain, self.confine);
    }

    fn reset_probe(&mut self) {
        self.callbacks_at_last_probe = self.host.callbacks();
        self.last_probe_point = None;
    }

    fn reset_recovery(&mut self) {
        self.recovery_attempts = 0;
        self.next_recovery_ms = 0;
        self.outage = false;
    }

    fn recover_if_due(&mut self, reason: &str) {
        let now = self.host.monotonic_ms();
        if self.recovery_attempts >= MAX_RECOVERY_ATTEMPTS || now < self.next_recovery_ms {
            return;
        }
        self.recovery_attempts += 1;
        let ok = self.host.recover();
        // A handle alone does not end the outage. Only new callbacks re-arm the budget.
        // Keep the pre-recovery baseline: callbacks delivered during install count on the next probe.
        let finished = self.host.monotonic_ms();
        self.next_recovery_ms = finished.saturating_add(RECOVERY_BACKOFF_MS << (self.recovery_attempts - 1));
        self.host.recovery_log(&format!(
            "hook recovery: reason={reason} attempt={}/{} installed={ok} next_delay_ms={} exhausted={}",
            self.recovery_attempts,
            MAX_RECOVERY_ATTEMPTS,
            self.next_recovery_ms.saturating_sub(finished),
            self.recovery_attempts == MAX_RECOVERY_ATTEMPTS
        ));
    }
}

impl<H: HookHost> GuardPort for Guard<H> {
    fn metrics(&self) -> Option<fence_types::HookMetrics> {
        self.host.metrics()
    }
    fn pause_trace(&mut self) {
        self.host.pause_trace();
    }
    fn configure_trace(
        &mut self,
        settings: &fence_config::Settings,
        monitors: &[fence_types::MonitorInfo],
        state: fence_types::RuntimeState,
    ) {
        self.host.configure_trace(settings, monitors, state);
    }
    fn drain_trace(&mut self, state: fence_types::RuntimeState) -> usize {
        self.host.drain_trace(state)
    }
    fn mark_accident(&mut self, state: fence_types::RuntimeState) -> usize {
        self.host.mark_accident(state)
    }
    fn configure(&mut self, config: EngineConfig) {
        self.host.set_engine_config(config);
    }

    fn start(&mut self) {
        self.running = true;
        self.reset_probe();
        self.reset_recovery();
        if self.host.installed() {
            return;
        }
        self.host.reset_engine();
        self.host.install();
    }

    fn stop(&mut self) {
        self.running = false;
        self.reset_probe();
        self.reset_recovery();
        if !self.host.installed() {
            return;
        }
        self.host.uninstall();
    }

    fn enabled(&self) -> bool {
        self.running
    }

    fn hook_installed(&self) -> bool {
        self.host.installed()
    }

    fn set_gate_open(&mut self, on: bool) {
        self.gate_open = on;
        self.push_toggles();
    }

    fn set_side_contain(&mut self, on: bool) {
        self.side_contain = on;
        self.push_toggles();
    }

    fn set_confine(&mut self, on: bool) {
        self.confine = on;
        self.push_toggles();
    }

    fn check_health(&mut self) -> HookHealthVerdict {
        if !self.running {
            return HookHealthVerdict::Inconclusive;
        }
        let mut moved = false;
        if let Some(now) = self.host.cursor_pos() {
            // The first probe after Start has no baseline: record it and claim no movement.
            moved = self.last_probe_point.is_some_and(|p| p != now);
            self.last_probe_point = Some(now);
        }

        // The hook-thread cursor query may itself pump pending callbacks. Count those too.
        let seen = self.host.callbacks();
        let delta = seen.wrapping_sub(self.callbacks_at_last_probe) as i64;
        self.callbacks_at_last_probe = seen;
        let verdict = hook_health::evaluate(moved, delta);
        if verdict == HookHealthVerdict::Alive {
            self.reset_recovery();
        } else {
            let missing = !self.host.installed();
            self.outage |= verdict == HookHealthVerdict::Dead || missing;
            if self.outage {
                self.recover_if_due(if missing {
                    "missing_handle"
                } else if moved {
                    "cursor_moved_without_callbacks"
                } else {
                    "awaiting_callbacks_after_outage"
                });
                return HookHealthVerdict::Dead;
            }
        }
        verdict
    }

    fn fault_counts(&self) -> (u64, u64) {
        self.host.fault_counts()
    }
}
