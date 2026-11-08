//! Wire format for reference requests and replay cases (JSON lines, one object per line).
//!
//! Request verbs (see tools/reference-cs/Program.cs, which mirrors these exactly):
//! - `run`: screens + settings + starting toggles + ops -> the compiled plan and one result per op.
//! - `normalize`: settings JSON text (+ monitors for the v0-v3 key migration) -> what 1.x would save.
//! - `keys`: WinForms `Keys.ToString()` names for virtual-key codes 0-255.
//! - `helper`: one pure C# helper (MapLayout, DisambiguateKeys, SanitizeExcluded, MigrateLayoutKeysToStable,
//!   MonitorLabel, IsArrowRotationHotkey, NormalizeMonitorName, AntiTrap, Adjacent, DeriveRoutes, SortAndNumber).
//!
//! Semantics of `run` (both sides implement them identically):
//! - screens are turned into monitors the way C# `MonitorInfo.All` does (SortAndNumber, then key
//!   disambiguation with raw identity `stable_id + "\n" + device`);
//! - the plan is built like `TrayApplicationContext.Configure` + `MouseGuard.Configure`;
//! - `move`: if paused or `!NeedsProcessing` the result is `Unprocessed` (the hook passes it without calling
//!   Decide, so the engine state does not change); else `Decide(x, y, gate_open, side_contain)`;
//! - `gate`/`side`/`confine` set the live switch; `pause` stops the hook; `resume` starts it and resets the
//!   engine (C# `Start` after `Stop`); `reset` is a reconfigure (C# `Configure` ends with `Reset`).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireScreen {
    /// Raw stable id before disambiguation (C# `StableIdFor` result).
    pub id: String,
    /// GDI name, e.g. `\\.\DISPLAY1`.
    pub device: String,
    pub l: i32,
    pub t: i32,
    pub r: i32,
    pub b: i32,
    pub primary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireToggles {
    pub gate_open: bool,
    pub side_contain: bool,
    pub confine: bool,
}

impl Default for WireToggles {
    /// 1.x startup with default settings: StartGateClosed = true, StartSideContainOn = true, no game mode.
    fn default() -> Self {
        Self {
            gate_open: false,
            side_contain: true,
            confine: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Move { x: i32, y: i32 },
    Gate { on: bool },
    Side { on: bool },
    Confine { on: bool },
    Pause,
    Resume,
    Reset,
}

/// A `run` request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunCase {
    pub verb: String,
    pub id: String,
    pub screens: Vec<WireScreen>,
    /// A schema-v4 settings JSON object as 1.x would load it, or null for `new Settings()`.
    pub settings: Option<serde_json::Value>,
    pub toggles: WireToggles,
    pub ops: Vec<Op>,
}

impl RunCase {
    pub fn new(id: impl Into<String>, screens: Vec<WireScreen>, ops: Vec<Op>) -> Self {
        Self {
            verb: "run".to_string(),
            id: id.into(),
            screens,
            settings: None,
            toggles: WireToggles::default(),
            ops,
        }
    }
}

/// One op result from `run` (moves only carry `act`, `x`, `y`; toggles echo `act = "Ok"`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpResult {
    pub i: usize,
    /// "Pass" | "Block" | "Unprocessed" | "Ok".
    pub act: String,
    /// Where the cursor ends: the clamp for Block, the target otherwise.
    pub x: i32,
    pub y: i32,
    pub on_top: bool,
    pub have_last: bool,
}

// ---------------------------------------------------------------- `core` verb (Track A, shared additive)
//
// Semantics: a C# `GuardCore` is built field by field, the way the 1.1.1 tests build it (no Configure, no
// Normalize). With `graph`, its transitions come from `BoundaryGraph.Compile(settings, screens, safety)` over
// the raw settings (the tests' `GraphCore`), and the response carries the compiled transitions, the sealed
// screens, `has_safety_demotions` and `validate` (`BoundaryGraph.ValidateNoSeal` over the same input). Each
// `move` reports where the cursor ends (`bx, by`; the target itself for Pass); other ops report `act = "Ok"`.

// `is_top` verb (Track A, shared additive): `{"verb": "is_top", "rect": [l, t, r, b]}` -> `{"result": bool}`,
// C# `GuardCore.IsTop`.

/// GuardCore fields. Rects are `[l, t, r, b]`; absent fields take the C# field defaults.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireCore {
    pub has_top: bool,
    pub barrier_y: i32,
    pub gates: Vec<[i32; 2]>,
    /// `[min, max, owner_t, owner_b]`.
    pub safety_gates: Vec<[i32; 4]>,
    pub monitors: Vec<[i32; 4]>,
    pub forbidden: Vec<[i32; 4]>,
    pub has_sides: bool,
    pub main: [i32; 4],
    pub deliberate_cross: bool,
    pub descent_routing: bool,
    /// `[top, landing]`.
    pub descent_routes: Vec<[[i32; 4]; 2]>,
    pub cross_min_up: i32,
    pub cross_slack: i32,
    pub side_cross_min: i32,
    pub side_cross_slack: i32,
    pub confine: bool,
}

impl Default for WireCore {
    /// C# `new GuardCore()`.
    fn default() -> Self {
        Self {
            has_top: false,
            barrier_y: 0,
            gates: Vec::new(),
            safety_gates: Vec::new(),
            monitors: Vec::new(),
            forbidden: Vec::new(),
            has_sides: false,
            main: [0; 4],
            deliberate_cross: true,
            descent_routing: false,
            descent_routes: Vec::new(),
            cross_min_up: 3,
            cross_slack: 5,
            side_cross_min: 3,
            side_cross_slack: 5,
            confine: false,
        }
    }
}

/// One compiler input screen (C# `BoundaryScreen`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireGraphScreen {
    pub id: String,
    pub rect: [i32; 4],
    pub primary: bool,
    pub excluded: bool,
}

/// Compile input for the `core` verb.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireGraph {
    /// Raw settings JSON (not normalized), or null for `new Settings()`.
    pub settings: Option<serde_json::Value>,
    pub screens: Vec<WireGraphScreen>,
    /// `applyGeometrySafety`.
    pub safety: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CoreOp {
    Move {
        x: i32,
        y: i32,
        gate: bool,
        side: bool,
    },
    SetLast {
        x: i32,
        y: i32,
    },
    Confine {
        on: bool,
    },
    Reset,
    /// A test assigning fields between moves: every field of `core` is assigned, without a Reset; the compiled
    /// transitions and the cursor state stay.
    Set {
        core: Box<WireCore>,
    },
}

/// A `core` request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoreCase {
    pub verb: String,
    pub id: String,
    pub core: WireCore,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<WireGraph>,
    pub ops: Vec<CoreOp>,
}

impl CoreCase {
    pub fn new(id: impl Into<String>, core: WireCore, ops: Vec<CoreOp>) -> Self {
        Self {
            verb: "core".to_string(),
            id: id.into(),
            core,
            graph: None,
            ops,
        }
    }
}

/// Convenience constructor for tests and generators.
pub fn screen(id: &str, device: &str, l: i32, t: i32, r: i32, b: i32, primary: bool) -> WireScreen {
    WireScreen {
        id: id.to_string(),
        device: device.to_string(),
        l,
        t,
        r,
        b,
        primary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_trace_case_round_trips_as_json_lines() {
        let case = RunCase::new(
            "c1",
            vec![screen("MON\\A", "\\\\.\\DISPLAY1", 0, 0, 1920, 1080, true)],
            vec![
                Op::Move { x: 5, y: 6 },
                Op::Gate { on: true },
                Op::Pause,
                Op::Resume,
                Op::Reset,
            ],
        );
        let line = serde_json::to_string(&case).unwrap();
        assert!(!line.contains('\n'));
        assert!(line.contains("\"op\":\"move\""));
        let back: RunCase = serde_json::from_str(&line).unwrap();
        assert_eq!(back, case);
    }
}
