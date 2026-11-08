//! Parity harness for MouseFence 2.0 (PLAN T20/T21).
//!
//! - `case`: the JSON-lines wire format shared with the C# reference (tools/reference-cs).
//! - `reference`: spawns the reference once and exchanges one JSON line per request.
//! - `corpus`: a seeded SplitMix64 generator for synthetic layouts, settings and move streams (no outside crates).
//!
//! Ownership: skeleton for the files below; Track A extends `corpus` and adds the engine diff tests
//! (`tests/engine_reference_diff.rs`); Track B adds `tests/config_reference_diff.rs`. The wire format is
//! additive-only (new optional fields), because the reference and both tracks read it.
#![forbid(unsafe_code)]

pub mod case;
pub mod corpus;
pub mod edge;
pub mod reference;
