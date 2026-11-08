//! The synthetic offset fixture of 22665ee: the void below the right monitor and the excluded dummy
//! (tests/Tests.cs:2914-2932). Every decided landing must be a usable, non-forbidden pixel.

use crate::rig::*;

/// tests/Tests.cs `Rig()`: left, top, main*, right; the dummy (3100,560,3600,930) is excluded.
fn fixture() -> Core {
    Core::new(Spec {
        has_top: true,
        barrier_y: 0,
        gates: vec![[16, 1584]],
        has_sides: true,
        main: [0, 0, 1600, 900],
        monitors: rects(&[
            (-1200, 45, 0, 765),
            (-300, -900, 1900, 0),
            (0, 0, 1600, 900),
            (1600, 40, 3100, 760),
        ]),
        forbidden: rects(&[(3100, 560, 3600, 930)]),
        ..Spec::default()
    })
}

fn lands_legal(last: (i32, i32), target: (i32, i32)) {
    let mut c = fixture();
    c.set_last(last.0, last.1);
    let r = c.mvs(target.0, target.1, true, true);
    assert!(c.legal(r.bx, r.by), "{r:?}");
}

scenarios! {
    fn p227_offset_fixture_drift_down_off_rights_bottom_into_the_void_lands_on_a_usable_monitor() {
        lands_legal((2400, 750), (2400, 850));
    }

    fn p228_offset_fixture_deep_eased_flick_to_the_voids_bottom_lands_legal() {
        lands_legal((2400, 750), (2500, 899));
    }

    fn p229_offset_fixture_ease_teleport_main_to_void_below_right_never_lands_in_the_void() {
        lands_legal((1300, 700), (2400, 850));
    }

    fn p230_offset_fixture_move_into_the_excluded_dummy_is_kept_out_usable_not_dummy() {
        lands_legal((3000, 700), (3350, 735));
    }

    fn p231_offset_fixture_teleport_deep_into_the_dummys_bottom_is_kept_out_lands_legal() {
        lands_legal((3000, 735), (3350, 900));
    }

    fn p232_offset_fixture_the_main_right_void_corner_move_lands_legal() {
        lands_legal((1599, 800), (1750, 850));
    }

    fn p233_offset_fixture_thin_void_above_right_y0_to_40_lands_legal() {
        lands_legal((2400, 42), (2400, 15));
    }
}
