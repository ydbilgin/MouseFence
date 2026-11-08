//! Track A parity tests: one test per `P`, `B` and `J` row of docs/rust2/parity.tsv owned by the engine.
//!
//! Each `pNNN_` test is the C# `Check` of tests/Tests.cs (public main 19b769c; the 14 rows marked
//! `renamed-in-22665ee` use the synthetic names and geometry of commit 22665ee), with the same setup, the same
//! moves and the same condition, run on the Rust engine. The scenarios are also listed in `SCENARIOS` so the
//! ignored reference replay (`reference.rs`) can run each one again and compare every recorded op with the C# engine.
//!
//! Plain run: `cargo test -p fence-engine --test parity`. Reference replay:
//! `MF_REFERENCE_EXE=<exe> cargo test -p fence-engine --test parity -- --ignored`.

/// Declares scenario tests and the list the reference replay walks. The source keeps the literal `fn pNNN_...`
/// the parity checker looks for.
macro_rules! scenarios {
    ($( $(#[$m:meta])* fn $name:ident() $body:block )*) => {
        $( $(#[$m])* #[test] fn $name() $body )*

        /// Every scenario of this module, for the reference replay.
        pub const SCENARIOS: &[(&str, fn())] = &[ $( (stringify!($name), $name as fn()) ),* ];
    };
}

mod rig;

/// The reference-diff helpers shared with crates/fence-trace/tests/engine_reference_diff.rs (one implementation of the
/// reference semantics on the Rust side).
#[path = "../../../fence-trace/tests/engine_common/mod.rs"]
mod engine_common;

mod barrier;
mod configure;
mod descent;
mod graph;
mod judged;
mod offset;
mod reference;
mod sides;
mod topology;
mod voids;
mod wall;

/// Every exact-parity scenario (the `J` tests in `judged` prove deliberate differences from 1.1.1 and are not
/// replayed against it).
pub fn all_scenarios() -> Vec<(&'static str, fn())> {
    [
        barrier::SCENARIOS,
        configure::SCENARIOS,
        descent::SCENARIOS,
        graph::SCENARIOS,
        offset::SCENARIOS,
        sides::SCENARIOS,
        topology::SCENARIOS,
        voids::SCENARIOS,
        wall::SCENARIOS,
    ]
    .concat()
}
