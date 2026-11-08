//! Pure topology helpers (port of C# `Core/GuardCore.Topology.cs`). Track A.
//!
//! Integer arithmetic wraps like the C# default `unchecked` context, so no input can panic here.

use fence_types::{Direction, Rect};

/// C# `GuardCore.SeamTol` (2 px): edge-equality tolerance for `adjacent`. It absorbs mixed-DPI 1 px seam
/// rounding and is far below any real gap.
pub const SEAM_TOL: i32 = 2;

/// C# `GuardCore.IsTop`: a monitor entirely at or above the desktop origin (`b <= 0`).
pub fn is_top(m: Rect) -> bool {
    m.b <= 0
}

/// C# `GuardCore.Adjacent(a, b, d)`: touching edge within SEAM_TOL and a strictly positive overlap.
pub fn adjacent(a: Rect, b: Rect, d: Direction) -> bool {
    let v = a.b.min(b.b).wrapping_sub(a.t.max(b.t)) > 0; // shared vertical extent (strictly positive)
    let h = a.r.min(b.r).wrapping_sub(a.l.max(b.l)) > 0; // shared horizontal extent (strictly positive)
    match d {
        Direction::Right => a.r.wrapping_sub(b.l).unsigned_abs() <= SEAM_TOL as u32 && v,
        Direction::Left => a.l.wrapping_sub(b.r).unsigned_abs() <= SEAM_TOL as u32 && v,
        Direction::Down => a.b.wrapping_sub(b.t).unsigned_abs() <= SEAM_TOL as u32 && h,
        Direction::Up => a.t.wrapping_sub(b.b).unsigned_abs() <= SEAM_TOL as u32 && h,
    }
}

/// One owner-bound safety gate: X span plus the owning screen's Y band (C# `SafetyGates` tuple).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SafetyGate {
    pub min: i32,
    pub max: i32,
    pub owner_t: i32,
    pub owner_b: i32,
}

impl SafetyGate {
    pub const fn new(min: i32, max: i32, owner_t: i32, owner_b: i32) -> Self {
        Self {
            min,
            max,
            owner_t,
            owner_b,
        }
    }
}

/// C# `GuardCore.AntiTrap(mons, topIdx)`: (indices to warn about, safety gates).
///
/// One pass over all monitors. A bottom screen with no Down/Left/Right neighbour is isolated (warn). The subset
/// that still has a monitor above gets a safety gate: the X overlap with the up-neighbours plus the screen's own
/// Y band. A screen isolated on all four sides warns but gets no gate.
pub fn anti_trap(mons: &[Rect], top_idx: &[usize]) -> (Vec<usize>, Vec<SafetyGate>) {
    let mut warn = Vec::new();
    let mut gates = Vec::new();
    for i in 0..mons.len() {
        if top_idx.contains(&i) {
            continue; // top screens roam free above the barrier
        }
        let mut non_up = false;
        let mut up = false;
        let mut up_min = i32::MAX;
        let mut up_max = i32::MIN;
        for j in 0..mons.len() {
            if i == j {
                continue;
            }
            if adjacent(mons[i], mons[j], Direction::Down)
                || adjacent(mons[i], mons[j], Direction::Left)
                || adjacent(mons[i], mons[j], Direction::Right)
            {
                non_up = true;
            }
            if adjacent(mons[i], mons[j], Direction::Up) {
                up = true;
                up_min = up_min.min(mons[i].l.max(mons[j].l));
                up_max = up_max.max(mons[i].r.min(mons[j].r));
            }
        }
        if non_up {
            continue; // a real side/down escape exists
        }
        warn.push(i); // isolated: the tray warns and names this screen
        if up && up_max > up_min {
            gates.push(SafetyGate::new(up_min, up_max, mons[i].t, mons[i].b));
        }
    }
    (warn, gates)
}
