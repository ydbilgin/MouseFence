//! Descent routing and route derivation (tests/Tests.cs:1095-1345).

use crate::rig::*;

const TOP: R4 = (-440, -1440, 3000, 0);
const MAIN: R4 = (0, 0, 2560, 1440);
const RIGHT: R4 = (2560, 0, 4480, 1080);

/// C# `OnTopCore(route, monitors...)`: a core already on top (crossed up through MAIN's column first).
fn on_top_core(route: (R4, R4), monitors: &[R4]) -> Core {
    let mut c = Core::new(Spec {
        has_top: true,
        barrier_y: 0,
        descent_routing: true,
        descent_routes: vec![[arr(route.0), arr(route.1)]],
        monitors: rects(monitors),
        gates: vec![[24, 2536]],
        ..Spec::default()
    });
    c.mv(1280, 0, true);
    c.mv(1280, -15, true); // deliberate cross -> on top
    c
}

fn routed(routes: &[(R4, R4)], monitors: &[R4], gates: &[(i32, i32)], routing: bool) -> Core {
    Core::new(Spec {
        has_top: true,
        barrier_y: 0,
        descent_routing: routing,
        descent_routes: routes.iter().map(|(t, l)| [arr(*t), arr(*l)]).collect(),
        monitors: rects(monitors),
        gates: gates.iter().map(|g| [g.0, g.1]).collect(),
        ..Spec::default()
    })
}

scenarios! {
    fn p081_descent_routing_overhang_into_side_clamps_x_to_linked_landing_edge() {
        let mut c = on_top_core((TOP, MAIN), &[TOP, MAIN, RIGHT]);
        c.mv(2800, -100, true); // roam over the overhang (above the right screen)
        let r = c.mv(2800, 10, true); // descend: lands on RIGHT instead of MAIN
        assert!(r.block() && r.bx == 2559 && r.by == 10 && !r.on_top, "{r:?}");
    }

    fn p082_descent_routing_overhang_into_side_clamps_x_to_linked_landing_edge_mirror_left() {
        let top_l = (-2000, -1440, 1000, 0);
        let right_l = (0, 0, 1000, 1080); // landing
        let left_l = (-2000, 0, 0, 1080); // wrong side under the overhang
        let mut c = routed(&[(top_l, right_l)], &[top_l, right_l, left_l], &[(24, 976)], true);
        c.mv(500, 0, true);
        c.mv(500, -15, true); // on top
        c.mv(-1000, -100, true); // roam over the left overhang
        let r = c.mv(-1000, 10, true); // descend onto LEFT
        assert!(r.block() && r.bx == 0 && !r.on_top, "{r:?}");
    }

    fn p083_descent_routing_disabled_preserves_ungated_descent() {
        let mut c = routed(&[(TOP, MAIN)], &[TOP, MAIN, RIGHT], &[(24, 2536)], false);
        c.mv(1280, 0, true);
        c.mv(1280, -15, true); // on top
        c.mv(2800, -100, true);
        let r = c.mv(2800, 10, true); // descends freely
        assert!(r.pass() && !r.on_top, "{r:?}");
    }

    fn p084_descent_routing_within_linked_landing_passes_unchanged() {
        let mut c = on_top_core((TOP, MAIN), &[TOP, MAIN, RIGHT]);
        c.mv(1500, -100, true);
        let r = c.mv(1500, 10, true); // lands inside MAIN
        assert!(r.pass() && !r.on_top, "{r:?}");
    }

    fn p085_descent_routing_void_is_contained_on_screen() {
        let mut c = on_top_core((TOP, MAIN), &[TOP, MAIN]); // RIGHT absent: X 2800 below TOP is a void
        c.mv(2800, -100, true);
        let r = c.mv(2800, 10, true);
        assert!(r.block() && r.by == -1 && r.on_top && c.legal(r.bx, r.by), "{r:?}");
    }

    fn p086_descent_routing_void_resolved_above_barrier_keeps_top_free_roam() {
        let mut c = on_top_core((TOP, MAIN), &[TOP, MAIN]);
        c.mv(2800, -100, true);
        c.mv(2800, 10, true);
        let up = c.mv(2800, -100, true);
        assert!(up.pass() && up.bx == 2800 && up.by == -100 && up.on_top, "{up:?}");
    }

    fn p087_descent_routing_lateral_roam_selects_current_top_route() {
        let (top_a, land_a) = ((0, -1440, 1200, 0), (0, 0, 1000, 1080));
        let (top_b, land_b) = ((1200, -1440, 2400, 0), (1400, 0, 2400, 1080));
        let wrong_a = (1000, 0, 1200, 1080); // wrong screen under TOPa's right overhang
        let mut c = routed(
            &[(top_a, land_a), (top_b, land_b)],
            &[top_a, land_a, top_b, land_b, wrong_a],
            &[(24, 976)],
            true,
        );
        c.mv(500, 0, true);
        c.mv(500, -15, true); // on top via LANDa column
        c.mv(1100, -100, true); // above TOPa's right overhang
        let r = c.mv(1100, 10, true); // descend onto WRONGa -> route A clamps to 999
        assert!(r.block() && r.bx == 999 && !r.on_top, "{r:?}");
    }

    fn p088_descent_routing_mixed_top_bottom_edges_uses_local_top_edge_clamps_x_keeps_on_top_above_barrier() {
        let top_shallow = (2560, -1440, 3000, -700);
        let land2 = (2560, -700, 2900, 1080);
        let wrong2 = (2900, -700, 3400, 1080);
        let mut c = routed(&[(top_shallow, land2)], &[top_shallow, land2, wrong2], &[(2580, 2980)], true);
        c.mv(2700, -1000, true); // first move -> on top
        c.mv(2950, -900, true);
        let r = c.mv(2950, -690, true);
        assert!(r.block() && r.bx == 2899 && r.on_top, "{r:?}");
    }

    fn p089_descent_routing_mixed_top_bottom_edges_free_roam_intact_above_the_barrier_after_a_shallow_top_route() {
        let top_shallow = (2560, -1440, 3000, -700);
        let land2 = (2560, -700, 2900, 1080);
        let wrong2 = (2900, -700, 3400, 1080);
        let mut c = routed(&[(top_shallow, land2)], &[top_shallow, land2, wrong2], &[(2580, 2980)], true);
        c.mv(2700, -1000, true);
        c.mv(2950, -900, true);
        c.mv(2950, -690, true);
        let up = c.mv(2900, -5, true);
        assert!(up.pass() && up.on_top, "{up:?}");
    }

    fn p090_descent_routing_deep_top_clamps_x_and_clears_on_top_below_barrier() {
        let top_deep = (2560, -700, 3000, 0);
        let land_d = (2560, 0, 2900, 1080);
        let wrong_d = (2900, 0, 3400, 1080);
        let mut c = routed(&[(top_deep, land_d)], &[top_deep, land_d, wrong_d], &[(2580, 2980)], true);
        c.mv(2700, 0, true);
        c.mv(2700, -15, true); // on top
        c.mv(2950, -300, true);
        let r = c.mv(2950, 10, true);
        assert!(r.block() && r.bx == 2899 && !r.on_top, "{r:?}");
    }

    fn p091_descent_routing_ambiguous_multiple_from_links_passes_unchanged() {
        let mut c = routed(&[], &[TOP, MAIN, RIGHT], &[(24, 2536)], true);
        c.mv(1280, 0, true);
        c.mv(1280, -15, true);
        c.mv(2800, -100, true);
        let r = c.mv(2800, 10, true);
        assert!(r.pass() && !r.on_top, "{r:?}");
    }

    fn p092_descent_routing_confine_overrides_route() {
        let mut c = Core::new(Spec {
            has_top: true,
            barrier_y: 0,
            descent_routing: true,
            confine: true,
            descent_routes: vec![[arr(TOP), arr(MAIN)]],
            monitors: rects(&[MAIN, RIGHT]),
            ..Spec::default()
        });
        c.mv(2800, 500, true); // baseline on RIGHT
        let r = c.mv(4500, 500, true); // leave RIGHT -> confine blocks
        assert!(r.block(), "{r:?}");
    }

    fn p093_descent_routing_side_from_device_not_below_top_no_route() {
        let side_landing = (2560, -200, 4480, 880); // Landing.T (-200) < Top.B (0)
        let emitted = side_landing.1 >= TOP.3; // the tray's SF-1 test
        let routes: Vec<(R4, R4)> = if emitted { vec![(TOP, side_landing)] } else { vec![] };
        let mut c = routed(&routes, &[TOP, MAIN], &[(24, 2536)], true);
        c.mv(1280, -200, true); // first move -> on top
        c.mv(1280, -100, true);
        let r = c.mv(1280, 10, true); // a centre descent into MAIN
        assert!(routes.is_empty() && r.pass() && !r.on_top, "routes={} {r:?}", routes.len());
    }

    fn p094_derive_a_valid_below_and_overlapping_link_emits_one_route() {
        let routes = derive_routes_rec(&[("MAIN", "TOP")], &[("MAIN", MAIN), ("TOP", TOP)], &["TOP"]);
        assert_eq!(routes, vec![(TOP, MAIN)]);
    }

    fn p095_derive_a_non_x_overlapping_link_emits_no_route_no_teleport_across_the_gap() {
        let routes = derive_routes_rec(&[("FAR", "TOP")], &[("FAR", (5000, 0, 6000, 1080)), ("TOP", TOP)], &["TOP"]);
        assert!(routes.is_empty(), "{routes:?}");
    }

    fn p096_derive_a_not_genuinely_below_landing_emits_no_route() {
        let routes =
            derive_routes_rec(&[("SIDE", "TOP")], &[("SIDE", (2560, -200, 4480, 880)), ("TOP", TOP)], &["TOP"]);
        assert!(routes.is_empty(), "{routes:?}");
    }

    fn p097_derive_an_ambiguous_top_multiple_distinct_from_devices_emits_no_route() {
        let routes = derive_routes_rec(
            &[("A", "TOP"), ("B", "TOP")],
            &[("A", (0, 0, 1500, 1440)), ("B", (1500, 0, 3000, 1440)), ("TOP", TOP)],
            &["TOP"],
        );
        assert!(routes.is_empty(), "{routes:?}");
    }

    fn p098_derive_a_link_whose_target_is_not_a_top_device_emits_no_route() {
        let routes = derive_routes_rec(&[("MAIN", "TOP")], &[("MAIN", MAIN), ("TOP", TOP)], &[]);
        assert!(routes.is_empty(), "{routes:?}");
    }
}
