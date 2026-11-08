//! Side containment: the soft barrier around main's left and right edges (tests/Tests.cs:2274-2450).

use crate::rig::*;

scenarios! {
    fn p168_side_needs_processing_is_true_with_only_has_sides_set() {
        let c = Core::new(Spec {
            has_sides: true,
            main: [0, 0, 2560, 1440],
            ..Spec::default()
        });
        assert!(c.needs_processing());
    }

    fn p169_side_on_deliberate_push_right_crosses() {
        let mut c = new_sides();
        c.mvs(2000, 500, true, true);
        let r = c.mvs(2700, 500, true, true);
        assert!(r.pass() && r.bx == 2700, "{r:?}");
    }

    fn p170_side_on_deliberate_push_left_crosses() {
        let mut c = new_sides();
        c.mvs(500, 500, true, true);
        let r = c.mvs(-200, 500, true, true);
        assert!(r.pass() && r.bx == -200, "{r:?}");
    }

    fn p171_side_on_slow_right_drift_blocked_clamped_to_2559_y_kept() {
        let mut c = new_sides();
        c.mvs(2559, 500, true, true);
        let r = c.mvs(2561, 500, true, true); // dx=2 -> drift
        assert!(r.block() && r.bx == 2559 && r.by == 500 && !r.on_top, "{r:?}");
    }

    fn p172_side_on_slow_left_drift_blocked_clamped_to_0_y_kept() {
        let mut c = new_sides();
        c.mvs(1, 500, true, true);
        let r = c.mvs(-1, 500, true, true);
        assert!(r.block() && r.bx == 0 && r.by == 500, "{r:?}");
    }

    fn p173_side_on_steep_diagonal_dx10_dy100_is_not_a_side_push_blocked() {
        let mut c = new_sides();
        c.mvs(2555, 500, true, true);
        let r = c.mvs(2565, 600, true, true);
        assert!(r.block() && r.bx == 2559, "{r:?}");
    }

    fn p174_side_on_slow_side_to_main_drift_blocked_clamped_to_minus_1_y_kept() {
        let mut c = new_sides();
        c.mvs(-1, 500, true, true); // on the left side screen, hugging main's left edge
        let r = c.mvs(1, 500, true, true);
        assert!(r.block() && r.bx == -1 && r.by == 500, "{r:?}");
    }

    fn p175_side_on_slow_side_to_main_drift_right_blocked_clamped_to_2560_y_kept() {
        let mut c = new_sides();
        c.mvs(2561, 500, true, true);
        let r = c.mvs(2559, 500, true, true);
        assert!(r.block() && r.bx == 2560 && r.by == 500, "{r:?}");
    }

    fn p176_side_on_deliberate_side_to_main_push_crosses() {
        let mut c = new_sides();
        c.mvs(-100, 500, true, true);
        let r = c.mvs(100, 500, true, true); // dx=200 -> deliberate
        assert!(r.pass() && r.bx == 100, "{r:?}");
    }

    fn p177_side_on_and_deliberate_off_side_to_main_drift_crosses() {
        let mut c = new_sides();
        c.set(|s| s.deliberate_cross = false);
        c.mvs(-1, 500, true, true);
        let r = c.mvs(1, 500, true, true);
        assert!(r.pass(), "{r:?}");
    }

    fn p178_side_sensitivity_dx4_blocked_when_side_cross_min_8() {
        let mut c = new_sides();
        c.set(|s| s.side_cross_min = 8);
        c.mvs(2558, 500, true, true);
        let r = c.mvs(2562, 500, true, true);
        assert!(r.block() && r.bx == 2559, "{r:?}");
    }

    fn p179_side_sensitivity_dx4_crosses_at_default_side_cross_min_3() {
        let mut c = new_sides();
        c.mvs(2558, 500, true, true);
        let r = c.mvs(2562, 500, true, true);
        assert!(r.pass() && r.bx == 2562, "{r:?}");
    }

    fn p180_side_on_roaming_within_a_side_screen_is_free() {
        let mut c = new_sides();
        c.mvs(-100, 500, true, true);
        let r = c.mvs(-300, 500, true, true);
        assert!(r.pass(), "{r:?}");
    }

    fn p181_side_on_up_left_corner_drift_contained_on_both_axes_bx0_by0() {
        let mut c = new_sides();
        c.mvs(1, 1, true, true);
        let r = c.mvs(-1, -1, true, true);
        assert!(r.block() && r.bx == 0 && r.by == 0, "{r:?}");
    }

    fn p182_side_on_a_centre_up_gate_crossing_still_works_no_interference() {
        let mut c = new_sides();
        c.mvs(1280, 0, true, true);
        let r = c.mvs(1280, -15, true, true);
        assert!(r.pass() && r.on_top, "{r:?}");
    }

    fn p183_side_on_and_deliberate_off_any_sideways_move_crosses() {
        let mut c = new_sides();
        c.set(|s| s.deliberate_cross = false);
        c.mvs(2000, 500, true, true);
        let r = c.mvs(2700, 510, true, true);
        assert!(r.pass(), "{r:?}");
    }

    fn p184_side_on_a_purely_vertical_move_inside_main_passes() {
        let mut c = new_sides();
        c.mvs(1280, 500, true, true);
        let r = c.mvs(1280, 800, true, true);
        assert!(r.pass(), "{r:?}");
    }

    fn p185_side_on_taller_side_screen_blocked_drift_stays_on_main_void_corrected() {
        let mut c = Core::new(Spec {
            has_top: false,
            barrier_y: 0,
            has_sides: true,
            main: [0, 0, 1000, 800],
            monitors: rects(&[(0, 0, 1000, 800), (1000, 0, 2000, 1000)]),
            ..Spec::default()
        });
        c.mvs(990, 790, true, true);
        let r = c.mvs(1010, 900, true, true); // steep -> blocked; Y=900 is void at the clamped X
        assert!(r.block() && r.bx == 999 && r.by == 799, "{r:?}");
    }

    fn p186_side_on_descent_into_a_below_main_screen_keeps_y_no_over_clamp_regression() {
        let mut c = Core::new(Spec {
            has_top: false,
            barrier_y: 0,
            has_sides: true,
            main: [0, 0, 2560, 1440],
            monitors: rects(&[(0, 0, 2560, 1440), (0, 1440, 2560, 2880)]),
            ..Spec::default()
        });
        c.mvs(2500, 1400, true, true);
        let r = c.mvs(2565, 1700, true, true);
        assert!(r.block() && r.bx == 2559 && r.by == 1700, "{r:?}");
    }

    fn p187_side_on_a_near_edge_up_gate_crossing_x_stays_in_main_still_crosses() {
        let mut c = new_sides();
        c.mvs(30, 0, true, true); // inside the gate and inside main
        let r = c.mvs(30, -15, true, true);
        assert!(r.pass() && r.on_top, "{r:?}");
    }

    fn p188_confine_precedence_game_mode_handles_the_block_before_side_containment() {
        let mut c = Core::new(Spec {
            has_top: false,
            confine: true,
            has_sides: true,
            main: [0, 0, 2560, 1440],
            monitors: rects(&[(0, 0, 2560, 1440), (2560, 0, 4480, 1080)]),
            ..Spec::default()
        });
        c.mvs(1000, 500, true, true);
        let r = c.mvs(3000, 500, true, true);
        assert!(r.block(), "{r:?}");
    }

    fn p189_side_off_sideways_drift_passes_freely_no_containment() {
        let mut c = new_sides();
        c.mvs(2559, 500, true, false);
        let r = c.mvs(2561, 500, true, false);
        assert!(r.pass() && r.bx == 2561, "{r:?}");
    }
}
