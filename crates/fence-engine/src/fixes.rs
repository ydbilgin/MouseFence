//! Judged fixes over 1.1.1 (the `J` rows of docs/rust2/parity.tsv). Track A.
//!
//! The port is exact first; each fix lands after exact parity in its own commit and can be switched off, so the
//! reference diff keeps proving the exact engine (`Fixes::EXACT_1_1_1`) while 2.0 ships with every fix on
//! (`Fixes::ALL`, the default). Each fix reports when it changed a result (`HIT_*` bits), which lets the reference
//! diff of the shipped engine attribute every difference from 1.1.1 to a named fix.

/// Which judged fixes are on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fixes {
    /// J01 (commit cd65f2e): a move that ends inside the source monitor (its top row and left column included,
    /// both inside the half-open rect) is not a crossing of that monitor's Up or Left transition.
    pub j01_source_edge_pixels: bool,
    /// J05 (PLAN F6): the seal check runs over strongly connected groups of screens, not single screens. 1.1.1
    /// flags a screen only when it has no usable exit at all, so two screens joined by a usable seam whose every
    /// way out is Blocked trap the cursor unnoticed: once there it never gets back to the primary. With the fix
    /// such a group is reported (one issue per group, named after the owner of its widest way out, which is the
    /// exit geometry safety demotes), and geometry safety repeats its demotion until no group is sealed (one
    /// demotion can join two sealed screens into a sealed pair). A group that holds the primary screen is never
    /// sealed (confining the cursor there is the fence's purpose; see `compile::sealed_groups`), and single
    /// screens keep the 1.1.1 check.
    pub j05_group_seal: bool,
    /// J06 (PLAN F7, lesson L8): a move that passes a transition may land beyond its target only along seams this
    /// move may cross: Free seams, and Push seams for which the move itself shows push intent. 1.1.1 authorizes
    /// every screen reachable from the target over seams that are not Blocked, so a shallow diagonal that crosses a
    /// Free side seam lands on the screen behind a Push Up seam (two hops away) without the push. Applies to a
    /// passing move and its void correction; the guided descent and the no-transition path keep 1.1.1.
    pub j06_downstream_intent: bool,
    /// J09 / F01: corridor ends hold; only the inset passage accepts a push.
    pub j09_full_corridor: bool,
    /// J10 / F02: every physical move starts at the observed cursor position.
    pub j10_observed_origin: bool,
    /// J11 / F03: traverse the actual segment through successive seams; durable display identities.
    pub j11_segment_and_identity: bool,
    pub j12_corner_void: bool,
    pub j16_shortcut_validation: bool,
    pub j17_win_capture: bool,
    pub j18_settings_durability: bool,
    pub j19_unknown_settings: bool,
    pub j20_error_log: bool,
    pub j21_ui_keepers: bool,
    pub j22_seed_after_panic: bool,
    pub j13_classic_1k: bool,
    pub j14_stop_then_push: bool,
    pub j15_side_to_top_blocked: bool,
    pub j24_edge_trace: bool,
    pub j25_fullscreen_quiet: bool,
    pub j26_hook_metrics: bool,
}

impl Fixes {
    /// The exact 1.1.1 behaviour (reference diff).
    pub const EXACT_1_1_1: Fixes = Fixes {
        j01_source_edge_pixels: false,
        j05_group_seal: false,
        j06_downstream_intent: false,
        j09_full_corridor: false,
        j10_observed_origin: false,
        j11_segment_and_identity: false,
        j12_corner_void: false,
        j16_shortcut_validation: false,
        j17_win_capture: false,
        j18_settings_durability: false,
        j19_unknown_settings: false,
        j20_error_log: false,
        j21_ui_keepers: false,
        j22_seed_after_panic: false,
        j13_classic_1k: false,
        j14_stop_then_push: false,
        j15_side_to_top_blocked: false,
        j24_edge_trace: false,
        j25_fullscreen_quiet: false,
        j26_hook_metrics: false,
    };
    /// Every judged fix (MouseFence 2.0).
    pub const ALL: Fixes = Fixes {
        j01_source_edge_pixels: true,
        j05_group_seal: true,
        j06_downstream_intent: true,
        j09_full_corridor: true,
        j10_observed_origin: true,
        j11_segment_and_identity: true,
        j12_corner_void: true,
        j16_shortcut_validation: true,
        j17_win_capture: true,
        j18_settings_durability: true,
        j19_unknown_settings: true,
        j20_error_log: true,
        j21_ui_keepers: true,
        j22_seed_after_panic: true,
        j13_classic_1k: true,
        j14_stop_then_push: true,
        j15_side_to_top_blocked: true,
        j24_edge_trace: true,
        j25_fullscreen_quiet: true,
        j26_hook_metrics: true,
    };
}

impl Default for Fixes {
    /// 2.0 ships with every judged fix.
    fn default() -> Self {
        Self::ALL
    }
}

/// J01 changed a decision (a transition 1.1.1 would have crossed was skipped).
pub const HIT_J01: u16 = 1 << 0;
/// J05 changed a compile or a validation (a sealed group of two or more screens was reported or demoted).
pub const HIT_J05: u16 = 1 << 1;
/// J06 refused a landing 1.1.1 would have authorized (a screen reached only over a Push seam without intent).
pub const HIT_J06: u16 = 1 << 2;
pub const HIT_J09: u16 = 1 << 3;
pub const HIT_J11: u16 = 1 << 4;
pub const HIT_J12: u16 = 1 << 5;
pub const HIT_J13: u16 = 1 << 6;
pub const HIT_J14: u16 = 1 << 7;
pub const HIT_J15: u16 = 1 << 8;
