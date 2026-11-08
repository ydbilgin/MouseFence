//! The void invariant: every returned point (Pass target or Block clamp) lies on a usable monitor
//! (tests/Tests.cs:615-812; P029-P031 and P035 use the synthetic names of 22665ee).

use crate::rig::*;

/// The synthetic four-display layout of 22665ee: the right monitor starts at Y=40 while the ultrawide above ends
/// at X=1900, so X>1900, Y<40 is a desktop void.
const SYN4: [R4; 4] = [
    (-1200, 45, 0, 765),
    (-300, -900, 1900, 0),
    (0, 0, 1600, 900),
    (1600, 40, 3100, 760),
];
const TOP: R4 = (-300, -900, 1900, 0);
const RIGHT_DISPLAY: R4 = (1600, 40, 3100, 760);

/// The seam-void repro of tests/Tests.cs:655: free roam across the top overhang, descend through the seam void,
/// then try upward twice at the same outside-gate X, then return to the gate.
fn seam_void() -> (Core, [M; 7]) {
    let mut c = Core::new(Spec {
        has_top: true,
        barrier_y: 0,
        gates: vec![[16, 1584]],
        monitors: rects(&SYN4),
        ..Spec::default()
    });
    c.set_last(700, 0);
    let cross_up = c.mv(700, -10, true);
    let roam_right = c.mv(1750, -10, true);
    let descend = c.mv(1750, 30, true);
    let up_again = c.mv(1750, -100, true);
    let up_again2 = c.mv(1750, -200, true);
    let return_to_gate = c.mv(1500, 40, true);
    let cross_up_again = c.mv(1500, -100, true);
    (
        c,
        [
            cross_up,
            roam_right,
            descend,
            up_again,
            up_again2,
            return_to_gate,
            cross_up_again,
        ],
    )
}

/// tests/Tests.cs:785 `Run(layout)` of the eight-monitor stress.
fn stress(layout: &[R4]) -> (Vec<(bool, i32, i32)>, bool) {
    let mut c = Core::new(Spec {
        has_top: true,
        barrier_y: 0,
        gates: vec![[20, 180]],
        has_sides: true,
        main: [0, 0, 200, 140],
        monitors: rects(layout),
        side_cross_min: 9,
        side_cross_slack: 4,
        ..Spec::default()
    });
    c.set_last(100, 70);
    let mut out = Vec::new();
    let mut legal = true;
    for i in 0..512 {
        let x = -700 + (i * 137 % 1501);
        let y = -420 + (i * 83 % 901);
        let r = c.mvs(x, y, (i & 1) == 0, (i % 3) != 0);
        legal &= c.legal(r.bx, r.by);
        out.push((r.block(), r.bx, r.by));
    }
    (out, legal)
}

scenarios! {
    fn p029_void_invariant_synthetic_right_corner_up_block_lands_on_a_usable_monitor() {
        let mut c = Core::new(Spec {
            has_top: true,
            barrier_y: 0,
            gates: vec![[16, 1584]],
            has_sides: true,
            main: [0, 0, 1600, 900],
            monitors: rects(&SYN4),
            ..Spec::default()
        });
        c.set_last(3098, 41); // on the right display, one pixel below its top-right corner
        let r = c.mvs(3105, -10, true, true); // up/right beyond it
        assert!(c.legal(r.bx, r.by), "{r:?}");
    }

    fn p030_void_invariant_synthetic_shallow_right_corner_pass_target_is_contained_on_screen() {
        let mut c = Core::new(Spec {
            has_top: true,
            barrier_y: 0,
            monitors: rects(&SYN4),
            ..Spec::default()
        });
        c.set_last(3098, 41);
        let r = c.mv(3099, 35, true); // shallow target above the right display, below the global barrier
        assert!(c.legal(r.bx, r.by), "{r:?}");
    }

    fn p031_seam_void_descent_nearest_legal_landing_is_down_on_right_display_never_top_y_minus_1() {
        let (_c, [_, _, descend, ..]) = seam_void();
        assert!(
            descend.block()
                && descend.bx == 1750
                && descend.by == RIGHT_DISPLAY.1
                && !descend.on_top
                && in_rect(RIGHT_DISPLAY, descend.bx, descend.by),
            "{descend:?}"
        );
    }

    fn p032_seam_void_descent_repeated_outside_gate_up_attempts_do_not_recreate_the_stale_fixpoint() {
        let (_c, [_, _, _, up1, up2, ..]) = seam_void();
        assert!(
            up1.block()
                && up1.bx == 1750
                && up1.by == RIGHT_DISPLAY.1
                && !up1.on_top
                && up2.block()
                && up2.bx == 1750
                && up2.by == RIGHT_DISPLAY.1
                && !up2.on_top,
            "{up1:?} {up2:?}"
        );
    }

    fn p033_seam_void_descent_cursor_can_return_to_the_gate_and_regain_consistent_top_free_roam() {
        let (_c, [.., ret, cross]) = seam_void();
        assert!(
            ret.pass()
                && !ret.on_top
                && cross.pass()
                && cross.bx == 1500
                && cross.by == -100
                && cross.on_top
                && in_rect(TOP, cross.bx, cross.by),
            "{ret:?} {cross:?}"
        );
    }

    fn p034_seam_void_descent_every_result_in_the_reproduced_sequence_lands_on_a_usable_monitor() {
        let (c, seq) = seam_void();
        assert!(seq.iter().all(|r| c.legal(r.bx, r.by)), "{seq:?}");
    }

    fn p035_void_invariant_synthetic_left_screen_outer_edge_remains_an_unchanged_legal_pass() {
        let mut c = Core::new(Spec {
            has_top: true,
            barrier_y: 0,
            monitors: rects(&SYN4),
            ..Spec::default()
        });
        c.set_last(-1199, 500);
        let r = c.mv(-1200, 500, true);
        assert!(r.pass() && r.bx == -1200 && r.by == 500 && c.legal(r.bx, r.by), "{r:?}");
    }

    fn p036_void_invariant_wider_lower_monitor_overhang_up_block_stays_on_its_entry_display() {
        let mut c = Core::new(Spec {
            has_top: true,
            barrier_y: 0,
            monitors: rects(&[(0, -100, 100, 0), (-50, 20, 150, 120)]),
            ..Spec::default()
        });
        c.set_last(140, 21);
        let r = c.mv(140, -10, false);
        assert!(r.block() && r.bx == 140 && r.by == 20 && c.legal(r.bx, r.by), "{r:?}");
    }

    fn p037_void_invariant_side_in_containment_across_a_gap_lands_on_the_origin_side_display() {
        let mut c = Core::new(Spec {
            has_sides: true,
            main: [0, 0, 100, 100],
            side_cross_min: 50,
            monitors: rects(&[(0, 0, 100, 100), (120, 0, 220, 100)]),
            ..Spec::default()
        });
        c.set_last(120, 50);
        let r = c.mvs(99, 50, true, true);
        assert!(r.block() && r.bx == 120 && r.by == 50 && c.legal(r.bx, r.by), "{r:?}");
    }

    fn p038_void_invariant_1_portrait_negative_origin_monitor_contains_a_corner_beyond_every_screen() {
        let mut c = Core::new(Spec {
            monitors: rects(&[(-300, -500, -100, 100)]),
            ..Spec::default()
        });
        let r = c.mv(900, 900, true);
        assert!(r.block() && r.bx == -101 && r.by == 99 && c.legal(r.bx, r.by), "{r:?}");
    }

    fn p039_void_invariant_2_monitor_gap_tie_is_legal_and_independent_of_monitor_enumeration() {
        let monitors = [(-300, -100, -200, 100), (0, -100, 100, 100)];
        let run = |layout: &[R4]| {
            let mut c = Core::new(Spec {
                monitors: rects(layout),
                ..Spec::default()
            });
            let r = c.mv(-100, 0, true); // centred in the 100 px gap: exact distance tie
            (r.act, r.bx, r.by)
        };
        let a = run(&monitors);
        let reversed: Vec<R4> = monitors.iter().rev().copied().collect();
        let b = run(&reversed);
        let check = Core::new(Spec {
            monitors: rects(&monitors),
            ..Spec::default()
        });
        assert!(a == b && a.0 == fence_types::Action::Block && check.legal(a.1, a.2), "forward={a:?} reverse={b:?}");
    }

    fn p040_void_invariant_8_monitor_mixed_layout_stress_terminates_stays_legal_and_is_deterministic() {
        let monitors = [
            (-420, -300, -300, 100),
            (-220, -180, 100, 0),
            (0, 0, 200, 140),
            (-200, 20, 0, 140),
            (220, 30, 420, 150),
            (0, 160, 140, 360),
            (500, -80, 580, 180),
            (-520, 220, -260, 300),
        ];
        let (forward, forward_legal) = stress(&monitors);
        let reversed: Vec<R4> = monitors.iter().rev().copied().collect();
        let (reverse, reverse_legal) = stress(&reversed);
        assert!(
            forward_legal
                && reverse_legal
                && forward == reverse
                && forward.iter().any(|r| r.0)
                && forward.iter().any(|r| !r.0),
            "forward_legal={forward_legal} reverse_legal={reverse_legal} same={}",
            forward == reverse
        );
    }
}
