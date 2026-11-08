//! The low-level mouse hook (port of C# `Platform/MouseGuard.cs`, hook side).
//!
//! - [`core`]: the hook step as a pure function over a [`fence_types::Decider`] and a [`core::CursorPort`]
//!   (B021-B023, B028; no Windows call, so every rule is unit-tested without a real hook).
//! - [`thread`]: the hook's own thread and message loop at normal priority (PLAN T9, B027), the real
//!   `WH_MOUSE_LL` callback that feeds the step, and the command channel the tray thread drives it with.
//!   Toggles reach the step through atomics in [`core::HookShared`]; the callback never waits on the tray.

pub mod core;
mod desktop;
mod raw;
pub mod thread;

pub use self::core::{CursorPort, HookCore, HookCounters, HookEvent, HookShared, StepOutcome};
pub use self::thread::{HookInstaller, HookThread, HookThreadInfo, LowLevelMouseHook, current_thread_id};
