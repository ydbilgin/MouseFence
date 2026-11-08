//! Hook liveness verdict (port of C# `Core/HookHealth.cs`, complete in the skeleton).
//!
//! Windows silently removes a WH_MOUSE_LL hook whose callback overruns LowLevelHooksTimeout and does not
//! clear the handle, so a non-zero handle proves nothing. The only evidence is traffic: the cursor moved
//! between two probes and not one callback arrived means the hook is gone. No movement is no evidence.

use fence_types::HookHealthVerdict;

/// C# `HookHealth.Evaluate`.
pub fn evaluate(cursor_moved: bool, callbacks_since_previous_probe: i64) -> HookHealthVerdict {
    if callbacks_since_previous_probe > 0 {
        return HookHealthVerdict::Alive;
    }
    if cursor_moved {
        HookHealthVerdict::Dead
    } else {
        HookHealthVerdict::Inconclusive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Parity rows P234-P238 (docs/rust2/parity.tsv), C# tests/Tests.cs:2944-2961.

    #[test]
    fn p234_hook_health_traffic_since_last_probe_is_alive() {
        assert_eq!(evaluate(true, 12), HookHealthVerdict::Alive);
    }

    #[test]
    fn p235_hook_health_traffic_without_cursor_movement_is_alive() {
        assert_eq!(evaluate(false, 3), HookHealthVerdict::Alive);
    }

    #[test]
    fn p236_hook_health_moved_without_callbacks_is_dead() {
        assert_eq!(evaluate(true, 0), HookHealthVerdict::Dead);
    }

    #[test]
    fn p237_hook_health_nothing_moved_nothing_arrived_is_inconclusive() {
        assert_eq!(evaluate(false, 0), HookHealthVerdict::Inconclusive);
    }

    #[test]
    fn p238_hook_health_zero_delta_on_nonzero_counter_is_not_death() {
        let total: i64 = 4321;
        let previous: i64 = 4321;
        assert_eq!(evaluate(false, total - previous), HookHealthVerdict::Inconclusive);
    }
}
