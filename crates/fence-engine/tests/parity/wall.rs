//! Wall-off: forbidden (excluded) rects, the region exit and the monitor-aware resolution
//! (tests/Tests.cs:1336-1780 and 1887).

use crate::rig::*;

/// The dummy under test: an 800x600 headless screen to the right of MAIN.
const DUMMY: R4 = (2560, 0, 3360, 600);

/// The synthetic four-display layout of 22665ee (a disconnected dummy at the right).
const SYN4: [R4; 4] = [
    (-1200, 45, 0, 765),
    (-300, -900, 1900, 0),
    (0, 0, 1600, 900),
    (1600, 40, 3100, 760),
];

fn outside(r: R4, x: i32, y: i32) -> bool {
    !in_rect(r, x, y)
}

/// The no-baseline deep landings of tests/Tests.cs:1699 (one check per edge).
fn deep_landing(x: i32, y: i32) {
    let real = (0, 0, 50, 50);
    let mut c = new_monitor_aware_wall(&[real], &[(100, 100, 200, 200)]);
    let r = c.mv(x, y, true); // no HaveLast
    assert!(r.block() && in_rect(real, r.bx, r.by) && c.legal(r.bx, r.by), "{r:?}");
}

/// tests/Tests.cs:1710 `Run(lastInside)`.
fn tie_run(last_inside: bool) -> (i32, i32, bool) {
    let left = (0, 100, 100, 140);
    let right = (201, 100, 301, 140);
    let mut c = new_monitor_aware_wall(&[right, left], &[(100, 100, 201, 200)]); // reverse input order
    if last_inside {
        c.set_last(150, 150);
    }
    let r = c.mv(150, 150, true);
    (r.bx, r.by, r.block() && c.legal(r.bx, r.by))
}

scenarios! {
    fn p099_wall_off_large_delta_entry_from_left_clamps_to_left_edge_no_far_side_tunnel() {
        let mut c = new_forbidden(&[DUMMY]);
        c.mv(2500, 300, true); // inside MAIN, left of the dummy
        let r = c.mv(3340, 300, true); // big flick deep into the dummy
        assert!(r.block() && r.bx == 2559 && r.by == 300, "{r:?}");
    }

    fn p100_wall_off_diagonal_first_crossed_edge_is_top_despite_larger_dx() {
        let mut c = new_forbidden(&[(100, 100, 200, 200)]);
        c.mv(0, 99, true); // left and above
        let r = c.mv(150, 150, true); // the top edge is crossed first
        assert!(r.block() && r.bx == 150 && r.by == 99, "{r:?}");
    }

    fn p101_wall_off_diagonal_mirror_first_crossed_edge_is_left_despite_larger_dy() {
        let mut c = new_forbidden(&[(100, 100, 200, 200)]);
        c.mv(99, 0, true);
        let r = c.mv(150, 150, true); // the left edge is crossed first
        assert!(r.block() && r.bx == 99 && r.by == 150, "{r:?}");
    }

    fn p102_wall_off_entry_from_left_clamp_x_to_l_minus_1_keep_y() {
        let mut c = new_forbidden(&[DUMMY]);
        c.mv(2500, 250, true);
        let r = c.mv(2600, 250, true);
        assert!(r.block() && r.bx == 2559 && r.by == 250, "{r:?}");
    }

    fn p103_wall_off_entry_from_top_clamp_y_to_t_minus_1_keep_x() {
        let mut c = new_forbidden(&[(2560, -600, 3360, 0)]);
        c.mv(2900, -650, true); // above the dummy
        let r = c.mv(2900, -300, true); // down into it across its top edge
        assert!(r.block() && r.by == -601 && r.bx == 2900, "{r:?}");
    }

    fn p104_wall_off_sliding_along_the_seam_outside_passes_untouched_no_sticking() {
        let mut c = new_forbidden(&[DUMMY]);
        c.mv(2559, 100, true);
        let a = c.mv(2559, 200, true);
        let b = c.mv(2559, 300, true);
        let d = c.mv(2559, 400, true);
        assert!(a.pass() && b.pass() && d.pass() && a.bx == 2559 && d.by == 400, "{a:?} {b:?} {d:?}");
    }

    fn p105_wall_off_empty_list_is_inert_pass_no_clamp() {
        let mut c = new_forbidden(&[]);
        let r = c.mv(3000, 300, true);
        assert!(r.pass() && r.bx == 3000 && r.by == 300, "{r:?}");
    }

    fn p106_wall_off_runs_before_the_barrier_dummy_entry_is_walled_not_gated() {
        let mut c = Core::new(Spec {
            has_top: true,
            barrier_y: 0,
            gates: vec![[24, 2536]],
            forbidden: rects(&[DUMMY]),
            ..Spec::default()
        });
        c.mv(2500, 300, true);
        let wall = c.mv(2600, 300, true);
        assert!(wall.block() && wall.bx == 2559 && wall.by == 300 && !wall.on_top, "{wall:?}");
    }

    fn p107_wall_off_a_normal_gate_cross_is_unaffected_by_the_wall() {
        let mut c2 = Core::new(Spec {
            has_top: true,
            barrier_y: 0,
            gates: vec![[24, 2536]],
            forbidden: rects(&[DUMMY]),
            ..Spec::default()
        });
        c2.mv(1280, 0, true);
        let cross = c2.mv(1280, -15, true);
        assert!(cross.pass() && cross.on_top, "{cross:?}");
    }

    fn p108_wall_off_composes_with_confine_walled_by_step_0_first() {
        let mut c = Core::new(Spec {
            has_top: false,
            confine: true,
            monitors: rects(&[(0, 0, 2560, 1440)]),
            forbidden: rects(&[DUMMY]),
            ..Spec::default()
        });
        c.mv(2500, 300, true);
        let r = c.mv(2600, 300, true);
        assert!(r.block() && r.bx == 2559 && r.by == 300, "{r:?}");
    }

    fn p109_wall_off_needs_processing_is_true_with_only_forbidden_set() {
        let c = new_forbidden(&[DUMMY]);
        assert!(c.needs_processing());
    }

    fn p110_wall_off_no_top_and_no_confine_still_walls_the_dummy() {
        let mut c = new_forbidden(&[DUMMY]);
        c.mv(2500, 300, true);
        let r = c.mv(2600, 300, true);
        assert!(r.block() && r.bx == 2559, "{r:?}");
    }

    fn p111_wall_off_needs_processing_is_false_when_nothing_is_set_hook_stays_out() {
        let c = Core::new(Spec::default());
        assert!(!c.needs_processing());
    }

    fn p112_wall_off_adjacent_dummies_clamped_point_is_outside_all_forbidden_rects() {
        let (d1, d2) = ((2560, 0, 3360, 600), (3360, 0, 4160, 600));
        let mut c = new_forbidden(&[d1, d2]);
        c.mv(2500, 300, true);
        let r = c.mv(3400, 300, true); // inside D2; entry from the left -> exit left of D1
        assert!(
            r.block() && outside(d1, r.bx, r.by) && outside(d2, r.bx, r.by) && r.bx == 2559 && r.by == 300,
            "{r:?}"
        );
    }

    fn p113_wall_off_fix_a_no_have_last_into_adjacent_pair_outside_both() {
        let (d1, d2) = ((0, 0, 100, 100), (100, 0, 200, 100));
        let mut c = new_forbidden(&[d1, d2]);
        let r = c.mv(99, 50, true); // first move (no Last) lands inside D1, adjacent to D2
        assert!(
            r.block() && outside(d1, r.bx, r.by) && outside(d2, r.bx, r.by) && r.bx == 200 && r.by == 50,
            "{r:?}"
        );
    }

    fn p114_wall_off_fix_a_last_inside_neighbour_move_into_the_other_outside_both_no_oscillation() {
        let (d1, d2) = ((0, 0, 100, 100), (100, 0, 200, 100));
        let mut c = new_forbidden(&[d1, d2]);
        c.mv(50, 50, true); // Last inside D1
        let r = c.mv(150, 50, true); // move into D2
        assert!(r.block() && outside(d1, r.bx, r.by) && outside(d2, r.bx, r.by), "{r:?}");
    }

    fn p115_wall_off_fix_a_three_rect_chain_deep_tunnel_from_left_exits_left_past_the_whole_region() {
        let mut c = new_forbidden(&[(0, 0, 100, 100), (100, 0, 200, 100), (200, 0, 300, 100)]);
        c.mv(-10, 50, true); // left of the whole chain
        let r = c.mv(280, 50, true); // deep flick into D3
        assert!(r.block() && outside((0, 0, 300, 100), r.bx, r.by) && r.bx == -1 && r.by == 50, "{r:?}");
    }

    fn p116_wall_off_fix_c_last_on_tl_corner_pixel_inside_nearest_edge_tie_picks_left() {
        let mut c = new_forbidden(&[(100, 100, 200, 200)]);
        c.mv(100, 100, true); // Last on the TL corner pixel, inside the half-open rect
        let r = c.mv(150, 150, true);
        assert!(r.block() && r.bx == 99 && r.by == 150, "{r:?}");
    }

    fn p117_wall_off_fix_c_t_equals_1_inclusive_target_on_top_row_tie_to_earlier_t_left_wall() {
        let mut c = new_forbidden(&[(100, 100, 200, 200)]);
        c.mv(0, 50, true);
        let r = c.mv(150, 100, true); // tY == 1.0 exactly (inclusive bound)
        assert!(r.block() && r.bx == 99 && r.by == 100, "{r:?}");
    }

    fn p118_wall_off_fix_c_equal_earliest_t_corner_entry_tie_picks_x() {
        let mut c = new_forbidden(&[(100, 100, 200, 200)]);
        c.mv(99, 99, true); // one px diagonally outside the TL corner
        let r = c.mv(150, 150, true); // tX == tY -> tie picks X
        assert!(r.block() && r.bx == 99 && r.by == 150, "{r:?}");
    }

    fn p119_wall_off_monitor_aware_synthetic_teleport_returns_to_monitor_4_never_the_void_below_it() {
        let monitor4 = SYN4[3];
        let mut c = new_monitor_aware_wall(&SYN4, &[(3100, 560, 3600, 930)]);
        c.set_last(3090, 700);
        let r = c.mv(3350, 850, true);
        assert!(r.block() && r.bx < 3100 && in_rect(monitor4, r.bx, r.by) && c.legal(r.bx, r.by), "{r:?}");
    }

    fn p120_wall_off_monitor_aware_seam_hit_keeps_entry_height_no_450px_vertical_jump() {
        let monitor4 = SYN4[3];
        let mut c = new_monitor_aware_wall(&SYN4, &[(3100, 560, 3600, 930)]);
        c.set_last(3096, 200);
        let r = c.mv(3350, 650, true); // eased deep target into the dummy
        assert!(
            r.block() && r.bx < 3100 && in_rect(monitor4, r.bx, r.by) && (r.by - 200).abs() <= 5 && c.legal(r.bx, r.by),
            "{r:?}"
        );
    }

    fn p121_wall_off_monitor_aware_dummy_right_entry_side_usable_neighbour_orthogonal_clamp() {
        let real = (0, 0, 100, 100);
        let mut c = new_monitor_aware_wall(&[real], &[(100, -50, 200, 150)]);
        c.set_last(90, 90);
        let r = c.mv(190, 140, true);
        assert!(r.block() && in_rect(real, r.bx, r.by) && r.bx == 99 && c.legal(r.bx, r.by), "{r:?}");
    }

    fn p122_wall_off_monitor_aware_dummy_left_entry_side_usable_neighbour_orthogonal_clamp() {
        let real = (100, 0, 200, 100);
        let mut c = new_monitor_aware_wall(&[real], &[(0, -50, 100, 150)]);
        c.set_last(110, 90);
        let r = c.mv(10, 140, true);
        assert!(r.block() && in_rect(real, r.bx, r.by) && r.bx == 100 && c.legal(r.bx, r.by), "{r:?}");
    }

    fn p123_wall_off_monitor_aware_dummy_above_entry_side_usable_neighbour_orthogonal_clamp() {
        let real = (0, 100, 100, 200);
        let mut c = new_monitor_aware_wall(&[real], &[(-50, 0, 150, 100)]);
        c.set_last(90, 110);
        let r = c.mv(140, 10, true);
        assert!(r.block() && in_rect(real, r.bx, r.by) && r.by == 100 && c.legal(r.bx, r.by), "{r:?}");
    }

    fn p124_wall_off_monitor_aware_dummy_below_entry_side_usable_neighbour_orthogonal_clamp() {
        let real = (0, 0, 100, 100);
        let mut c = new_monitor_aware_wall(&[real], &[(-50, 100, 150, 200)]);
        c.set_last(90, 90);
        let r = c.mv(140, 190, true);
        assert!(r.block() && in_rect(real, r.bx, r.by) && r.by == 99 && c.legal(r.bx, r.by), "{r:?}");
    }

    fn p125_wall_off_monitor_aware_corner_dummy_prefers_the_real_monitor_the_move_came_from() {
        let left = (0, 100, 100, 160);
        let above = (100, 0, 160, 100);
        let mut c = new_monitor_aware_wall(&[left, above], &[(100, 100, 200, 200)]);
        c.set_last(90, 150);
        let r = c.mv(190, 190, true);
        assert!(
            r.block() && in_rect(left, r.bx, r.by) && !in_rect(above, r.bx, r.by) && c.legal(r.bx, r.by),
            "{r:?}"
        );
    }

    fn p126_wall_off_monitor_aware_shorter_offset_dummy_still_returns_to_its_taller_real_neighbour() {
        let real = (0, 0, 100, 200);
        let mut c = new_monitor_aware_wall(&[real], &[(100, 150, 200, 250)]);
        c.set_last(90, 190);
        let r = c.mv(190, 240, true);
        assert!(
            r.block() && in_rect(real, r.bx, r.by) && r.bx == 99 && r.by == 190 && c.legal(r.bx, r.by),
            "{r:?}"
        );
    }

    fn p127_wall_off_monitor_aware_adjacent_dummies_march_as_one_region_onto_the_real_screen_beyond_both() {
        let real = (300, 0, 400, 100);
        let mut c = new_monitor_aware_wall(&[real], &[(100, -50, 200, 150), (200, -50, 300, 150)]);
        c.set_last(310, 90);
        let r = c.mv(110, 140, true);
        assert!(
            r.block() && in_rect(real, r.bx, r.by) && r.bx == 300 && r.by == 90 && c.legal(r.bx, r.by),
            "{r:?}"
        );
    }

    fn p128_wall_off_monitor_aware_no_baseline_deep_landing_near_left_edge_resolves_on_screen() {
        deep_landing(101, 150);
    }

    fn p129_wall_off_monitor_aware_no_baseline_deep_landing_near_right_edge_resolves_on_screen() {
        deep_landing(199, 150);
    }

    fn p130_wall_off_monitor_aware_no_baseline_deep_landing_near_top_edge_resolves_on_screen() {
        deep_landing(150, 101);
    }

    fn p131_wall_off_monitor_aware_no_baseline_deep_landing_near_bottom_edge_resolves_on_screen() {
        deep_landing(150, 199);
    }

    fn p132_wall_off_monitor_aware_nearest_monitor_tie_is_deterministic_and_independent_of_monitor_list_order() {
        let a = tie_run(false);
        let b = tie_run(false);
        let left = (0, 100, 100, 140);
        let right = (201, 100, 301, 140);
        let mut repeat = new_monitor_aware_wall(&[right, left], &[(100, 100, 201, 200)]);
        let first = repeat.mv(150, 150, true);
        let second = repeat.mv(150, 150, true);
        let repeats = first.bx == second.bx && first.by == second.by && repeat.legal(second.bx, second.by);
        assert!(a == b && a.2 && a.0 == 99 && a.1 == 139 && repeats, "a={a:?} b={b:?} {first:?} {second:?}");
    }

    fn p133_wall_off_monitor_aware_last_inside_forbidden_region_falls_back_to_nearest_legal_monitor() {
        let inside = tie_run(true);
        assert!(inside.2 && inside.0 == 99 && inside.1 == 139, "{inside:?}");
    }

    fn p134_wall_off_monitor_aware_pathological_dummy_ring_terminates_on_its_usable_centre() {
        let centre = (100, 100, 200, 200);
        let ring = [(0, 0, 100, 300), (100, 0, 200, 100), (200, 0, 300, 300), (100, 200, 200, 300)];
        let mut c = new_monitor_aware_wall(&[centre], &ring);
        let r = c.mv(250, 250, true);
        assert!(r.block() && in_rect(centre, r.bx, r.by) && c.legal(r.bx, r.by), "{r:?}");
    }

    fn p135_wall_off_topology_excluded_device_absent_no_spurious_isolation_gate_right_screen_unaffected() {
        let with_dummy = [
            (-1920, 0, 0, 1080),
            (0, 0, 2560, 1440),
            (2560, 0, 4480, 1080),
            (-450, -1440, 2990, 0),
            (4480, 0, 5280, 600), // the dummy abutting the right screen
        ];
        let dropped: Vec<R4> = with_dummy.iter().enumerate().filter(|(i, _)| *i != 4).map(|(_, m)| *m).collect();
        let (warn, gates) = anti_trap_rec(&dropped, &[3]);
        assert!(warn.is_empty() && gates.is_empty(), "warn={warn:?} gates={gates:?}");
    }

    fn p142_wall_off_clone_overlap_usable_clone_geometry_wins_and_the_first_event_stays_on_screen() {
        let clone = (0, 0, 100, 100);
        let mut c = new_monitor_aware_wall(&[clone], &[clone]);
        let r = c.mv(50, 50, true);
        assert!(r.pass() && r.bx == 50 && r.by == 50 && in_rect(clone, r.bx, r.by), "{r:?}");
    }
}
