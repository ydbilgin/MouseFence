//! MouseFence decision engine: a faithful port of the C# 1.1.1 `Core/` (GuardCore + BoundaryGraph), proven
//! against the C# code itself through tools/reference-cs (PLAN T1).
//!
//! Module ownership (docs/rust2/CONVENTIONS.md):
//! - Track A: `compile`, `decide`, `fixes`, `plan`, `topology`, `routes`, `ids`, and the tests named `pNNN_`, `bNNN_`
//!   and `jNN_` for its rows
//!   (`tests/parity/`). `fixes` holds the judged fixes over 1.1.1 (J rows), on by default, off for the reference diff.
//! - Track B: `layout` (identity, exclusion sanitizing, key migration, map layout, labels, hotkey clash).
//! - Skeleton: `hook_health` (complete).
//!
//! Rules: `#![forbid(unsafe_code)]`; no allocation, lock, log or I/O inside `Decider::decide`; time never
//! read here (callers pass it in if a later policy needs it).
#![forbid(unsafe_code)]

pub mod compile;
pub mod decide;
pub mod fixes;
pub mod hook_health;
pub mod ids;
pub mod intent;
pub mod intent_detector;
pub mod layout;
pub mod plan;
pub mod routes;
pub mod topology;

pub use fence_types as types;
