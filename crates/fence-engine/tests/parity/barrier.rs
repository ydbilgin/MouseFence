//! The up-barrier, gates, game mode, deliberate crossing and safety gates (tests/Tests.cs:123 and 814-1050).

use crate::rig::*;

scenarios! {
    fn p001_right_screen_void_up_into_void_is_blocked_and_clamped_to_y0() {
        // At the top of DISPLAY2, moving up-right into the void (X>3000) is blocked and clamped to Y=0, not
        // snapped onto the top monitor's far wall.
        let mut c = new_core();
        c.mv(3500, 0, true);
        let r = c.mv(3600, -20, true);
        assert!(r.block() && r.by == 0 && !r.on_top, "{r:?}");
    }

    fn p041_right_screen_overlap_up_is_blocked() {
        let mut c = new_core();
        c.mv(2700, 0, true);
        let r = c.mv(2700, -10, true);
        assert!(r.block() && r.by == 0 && !r.on_top, "{r:?}");
    }

    fn p042_right_screen_slide_and_jitter_stays_on_bottom_row() {
        let mut c = new_core();
        c.mv(3000, 0, true);
        let a = c.mv(3100, 0, true);
        let b = c.mv(3200, -1, true);
        let d = c.mv(3300, 0, true);
        assert!(a.pass() && b.block() && d.pass() && !a.on_top && !b.on_top && !d.on_top, "{a:?} {b:?} {d:?}");
    }

    fn p043_side_left_up_is_blocked() {
        let mut c = new_core();
        c.mv(-500, 50, true);
        let r = c.mv(-500, -10, true);
        assert!(r.block() && !r.on_top, "{r:?}");
    }

    fn p044_main_gate_deliberate_up_push_crosses() {
        let mut c = new_core();
        c.mv(1280, 0, true);
        let r = c.mv(1280, -15, true);
        assert!(r.pass() && r.on_top, "{r:?}");
    }

    fn p045_main_gate_up_right_deliberate_push_crosses() {
        let mut c = new_core();
        c.mv(1280, 0, true);
        let r = c.mv(1290, -15, true); // dx=10, dyUp=15 -> 10 <= 23
        assert!(r.pass() && r.on_top, "{r:?}");
    }

    fn p046_main_slide_small_up_jitter_blocked() {
        let mut c = new_core();
        c.mv(1280, 0, true);
        let r = c.mv(1340, -2, true); // dyUp=2 < CrossMinUp
        assert!(r.block() && !r.on_top, "{r:?}");
    }

    fn p047_main_slide_fast_diagonal_blocked() {
        let mut c = new_core();
        c.mv(1280, 0, true);
        let r = c.mv(1400, -10, true); // dyUp=10, dx=120 -> 120 > 18
        assert!(r.block() && !r.on_top, "{r:?}");
    }

    fn p048_gate_closed_main_up_push_blocked() {
        let mut c = new_core();
        c.mv(1280, 0, false);
        let r = c.mv(1280, -15, false);
        assert!(r.block() && !r.on_top, "{r:?}");
    }

    fn p049_inset_up_at_far_left_of_main_blocked() {
        let mut c = new_core();
        c.mv(10, 0, true);
        let r = c.mv(10, -15, true);
        assert!(r.block(), "{r:?}");
    }

    fn p050_inset_up_at_far_right_of_main_blocked() {
        let mut c = new_core();
        c.mv(2550, 0, true);
        let r = c.mv(2550, -15, true);
        assert!(r.block(), "{r:?}");
    }

    fn p051_roam_on_top_is_free_descending_escapes() {
        let mut c = new_core();
        c.mv(1280, 0, true);
        c.mv(1280, -15, true); // now on top
        let roam1 = c.mv(1500, -200, true);
        let roam2 = c.mv(2900, -50, true); // over the part above DISPLAY2
        let down = c.mv(2900, 5, true); // push down -> escape to DISPLAY2
        assert!(
            roam1.pass() && roam1.on_top && roam2.pass() && roam2.on_top && down.pass() && !down.on_top,
            "{roam1:?} {roam2:?} {down:?}"
        );
    }

    fn p052_bottom_row_horizontal_move_passes() {
        let mut c = new_core();
        c.mv(1280, 500, true);
        let r = c.mv(1700, 500, true);
        assert!(r.pass() && !r.on_top, "{r:?}");
    }

    fn p053_side_origin_diagonal_from_display2_is_blocked() {
        let mut c = new_core();
        c.mv(2600, 0, true); // origin on DISPLAY2 (a side screen)
        let r = c.mv(2500, -200, true); // fast diagonal landing in the gate column, going up
        assert!(r.block() && r.by == 0 && !r.on_top, "{r:?}");
    }

    fn p054_side_origin_diagonal_from_display3_is_blocked() {
        let mut c = new_core();
        c.mv(-100, 30, true); // origin on DISPLAY3 (left side screen)
        let r = c.mv(100, -200, true); // diagonal up into the gate column
        assert!(r.block() && !r.on_top, "{r:?}");
    }

    fn p055_main_origin_deliberate_cross_still_works_origin_aware() {
        let mut c = new_core();
        c.mv(1280, 0, true);
        let r = c.mv(1280, -15, true);
        assert!(r.pass() && r.on_top, "{r:?}");
    }

    fn p056_slow_slide_and_jitter_dx10_dy3_blocked_with_slack_5() {
        let mut c = new_core();
        c.mv(1280, 0, true);
        let r = c.mv(1290, -3, true); // dx=10, dyUp=3 -> 10 > 3+5 -> Block
        assert!(r.block() && !r.on_top, "{r:?}");
    }

    fn p057_near_vertical_push_still_crosses_with_slack_5() {
        let mut c = new_core();
        c.mv(1280, 0, true);
        let r = c.mv(1283, -10, true); // dx=3, dyUp=10 -> 3 <= 15 -> Pass
        assert!(r.pass() && r.on_top, "{r:?}");
    }

    fn p058_multi_gate_deliberate_up_in_gate_a_crosses() {
        let mut c = new_gates(&[(0, 1000), (2000, 3000)]);
        c.mv(500, 0, true);
        let r = c.mv(500, -15, true);
        assert!(r.pass() && r.on_top, "{r:?}");
    }

    fn p059_multi_gate_deliberate_up_in_gate_b_crosses() {
        let mut c = new_gates(&[(0, 1000), (2000, 3000)]);
        c.mv(2500, 0, true);
        let r = c.mv(2500, -15, true);
        assert!(r.pass() && r.on_top, "{r:?}");
    }

    fn p060_multi_gate_up_between_gates_is_blocked() {
        let mut c = new_gates(&[(0, 1000), (2000, 3000)]);
        c.mv(1500, 0, true); // column between the two gates
        let r = c.mv(1500, -15, true);
        assert!(r.block() && !r.on_top, "{r:?}");
    }

    fn p061_multi_gate_cross_gate_diagonal_blocked_origin_aware() {
        let mut c = new_gates(&[(0, 1000), (2000, 3000)]);
        c.mv(500, 0, true); // origin in gate A
        let r = c.mv(2500, -200, true); // landing in gate B
        assert!(r.block() && !r.on_top, "{r:?}");
    }

    fn p062_game_mode_move_within_active_monitor_passes() {
        let mut c = new_confine(&[(0, 0, 2560, 1440), (2560, 0, 4480, 1080)]);
        c.mv(1000, 500, true);
        let inside = c.mv(2000, 600, true);
        assert!(inside.pass(), "{inside:?}");
    }

    fn p063_game_mode_leaving_active_monitor_is_blocked() {
        let mut c = new_confine(&[(0, 0, 2560, 1440), (2560, 0, 4480, 1080)]);
        c.mv(1000, 500, true);
        c.mv(2000, 600, true);
        let cross = c.mv(3000, 500, true);
        assert!(cross.block(), "{cross:?}");
    }

    fn p064_game_mode_leaving_upward_is_also_blocked() {
        let mut c = new_confine(&[(0, 0, 2560, 1440), (2560, 0, 4480, 1080)]);
        c.mv(1000, 500, true);
        let up = c.mv(1000, -10, true);
        assert!(up.block(), "{up:?}");
    }

    fn p065_deliberate_off_any_upward_move_through_the_gate_crosses() {
        let mut c = new_core();
        c.set(|s| s.deliberate_cross = false);
        c.mv(1280, 0, true);
        let r = c.mv(1290, -3, true); // would be blocked when deliberate is on
        assert!(r.pass() && r.on_top, "{r:?}");
    }

    fn p066_reconfigure_correct_barrier_blocks_the_side_screen_up_leak_at_the_seam() {
        let mut c = new_core(); // BarrierY = 0 (the live seam), gate [24,2536]
        c.mv(3000, 50, true);
        let r = c.mv(3000, -3, true);
        assert!(r.block() && r.by == 0 && !r.on_top, "{r:?}");
    }

    fn p067_reconfigure_a_stale_barrier_leaks_the_regression_auto_reconfigure_prevents() {
        let mut c = new_gates(&[(24, 2536)]);
        c.set(|s| s.barrier_y = -6); // stale barrier from a previous layout
        c.reset();
        c.mv(3000, 50, true);
        let r = c.mv(3000, -3, true); // y=-3 >= stale -6 -> a normal move -> leaks up
        assert!(r.pass(), "{r:?}");
    }

    fn p068_safety_sole_up_exit_crosses_even_when_crossing_is_closed() {
        let mut c = new_safety(&[(2830, 3000, 0, 1080)]);
        c.mv(2900, 0, false); // origin on the owner screen, crossing closed
        let r = c.mv(2900, -15, false);
        assert!(r.pass() && r.on_top, "{r:?}");
    }

    fn p069_safety_slide_jitter_does_not_leak_through_the_safety_gate() {
        let mut c = new_safety(&[(2830, 3000, 0, 1080)]);
        c.mv(2900, 0, false);
        let r = c.mv(2960, -2, false); // dx=60, dyUp=2 -> slide/jitter
        assert!(r.block() && !r.on_top, "{r:?}");
    }

    fn p070_safety_a_screen_in_the_same_x_column_but_different_y_cant_cross_the_safety_gate() {
        let mut c = new_safety(&[(2830, 3000, 0, 1080)]);
        c.mv(2900, 2500, false); // origin below the owner's Y band
        let r = c.mv(2900, -5, false);
        assert!(r.block() && !r.on_top, "{r:?}");
    }

    fn p071_safety_independent_of_user_gates_safety_passes_user_gate_blocks_toggle_closed() {
        let spec = || Spec {
            has_top: true,
            barrier_y: 0,
            gates: vec![[24, 2536]],
            safety_gates: vec![[2830, 3000, 0, 1080]],
            ..Spec::default()
        };
        let mut c = Core::new(spec());
        c.mv(2900, 0, false);
        let s = c.mv(2900, -15, false);
        let mut c2 = Core::new(spec());
        c2.mv(1280, 0, false);
        let u = c2.mv(1280, -15, false);
        assert!(s.pass() && u.block(), "safety={s:?} user={u:?}");
    }
}
