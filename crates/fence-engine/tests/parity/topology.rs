//! Anti-trap topology: pure detection from rects (tests/Tests.cs:1054-1094).

use crate::rig::*;
use fence_engine::topology::SafetyGate;
use fence_types::Direction;

scenarios! {
    fn p072_topology_the_270px_gap_right_screen_is_the_only_isolated_one() {
        // left, MAIN, right (270 px gap from MAIN), top: only the right screen is isolated.
        let mons = [(-1920, 0, 0, 1080), (0, 0, 2560, 1440), (2830, 0, 4480, 1080), (-450, -1440, 2990, 0)];
        let (warn, _) = anti_trap_rec(&mons, &[3]);
        assert_eq!(warn, vec![2]);
    }

    fn p073_topology_its_safety_gate_is_clipped_to_the_up_overlap_owner_bound() {
        let mons = [(-1920, 0, 0, 1080), (0, 0, 2560, 1440), (2830, 0, 4480, 1080), (-450, -1440, 2990, 0)];
        let (_, gates) = anti_trap_rec(&mons, &[3]);
        assert_eq!(gates, vec![SafetyGate::new(2830, 2990, 0, 1080)]);
    }

    fn p074_topology_edge_touching_right_screen_is_not_isolated() {
        let mons = [(-1920, 0, 0, 1080), (0, 0, 2560, 1440), (2560, 0, 4480, 1080), (-450, -1440, 2990, 0)];
        let (warn, gates) = anti_trap_rec(&mons, &[3]);
        assert!(warn.is_empty() && gates.is_empty(), "warn={warn:?}");
    }

    fn p075_topology_a_1px_seam_is_tolerated_seamtol_not_isolated() {
        let mons = [(-1920, 0, 0, 1080), (0, 0, 2560, 1440), (2561, 0, 4480, 1080), (-450, -1440, 2990, 0)];
        let (warn, _) = anti_trap_rec(&mons, &[3]);
        assert!(warn.is_empty(), "warn={warn:?}");
    }

    fn p076_topology_a_four_sided_island_warns_but_yields_no_safety_gate() {
        let mons = [(-1920, 0, 0, 1080), (0, 0, 2560, 1440), (5000, 5000, 6000, 6000), (-450, -1440, 2990, 0)];
        let (warn, gates) = anti_trap_rec(&mons, &[3]);
        assert!(warn.contains(&2) && gates.is_empty(), "warn={warn:?} gates={gates:?}");
    }

    fn p077_topology_zero_width_corner_touch_is_not_adjacency() {
        let (a, b) = ((0, 0, 100, 100), (100, 100, 200, 200)); // touch only at the corner
        assert!(!adjacent_rec(a, b, Direction::Right) && !adjacent_rec(a, b, Direction::Down));
    }

    fn p078_topology_partial_vertical_overlap_is_right_adjacency() {
        let (a, b) = ((0, 0, 100, 100), (100, 50, 200, 300)); // share [50,100) vertically
        assert!(adjacent_rec(a, b, Direction::Right));
    }

    fn p079_topology_a_top_set_screen_is_never_in_warn_gates() {
        let mons = [(0, 0, 2560, 1440), (-450, -1440, 2990, 0)];
        let (warn, _) = anti_trap_rec(&mons, &[1]);
        assert!(!warn.contains(&1), "warn={warn:?}");
    }

    fn p080_topology_is_top_classifies_an_above_origin_monitor() {
        assert!(is_top_rec((-450, -1440, 2990, 0)) && !is_top_rec((0, 0, 2560, 1440)));
    }
}
