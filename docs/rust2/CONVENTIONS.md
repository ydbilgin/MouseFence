# MouseFence 2.0 (Rust) working conventions

The Rust workspace is based on public main `68ea977`. Its parity reference is pinned to the `v1.1.1` tag.

## Goal in one line

MouseFence 2.0 is a Rust port of 1.1.1 that keeps every behaviour. `docs/rust2/parity.tsv` lists each
behaviour and the Rust test that proves it. The retired C# application remains in git history. The
reference sources are extracted from the pinned `v1.1.1` tag into ignored `reference-src/v1.1.1`.

## Who owns what

Each track changes only its own files. A file marked SHARED takes additive changes only (new items,
never edits to another track's items), and the commit message says "shared, additive".

| Track | Owns |
|---|---|
| A engine | `crates/fence-engine` except `src/layout.rs`; `crates/fence-trace/src/corpus.rs`; engine reference-diff tests in `crates/fence-trace/tests/` named `engine_*.rs` |
| B config + layout helpers | `crates/fence-config`; `crates/fence-engine/src/layout.rs`; config reference-diff tests `crates/fence-trace/tests/config_*.rs` |
| C Windows + tray | `crates/fence-win`, `crates/fence-app`, `apps/mousefence` |
| D settings window | `crates/fence-settings-ui` |
| SHARED, additive | `crates/fence-types`; `crates/fence-i18n/src/en.rs` and `tr.rs` (a key goes into both in the same commit); `crates/fence-trace/src/case.rs` (wire format); `tools/reference-cs/Program.cs` (new verbs only); `docs/rust2/parity.tsv` (rows of your own track); the root `Cargo.toml` (`[workspace.dependencies]` lines only) |

Historical C# paths (`Core/`, `Ui/`, `Platform/`, `Settings.cs`, `Program.cs`, `I18n/`, `tests/`) in parity
references describe the pinned tag. Never change its extracted runtime sources to alter an reference answer.

## Rules that hold for every track

- The decision logic stays pure: `fence-engine` has no Windows calls, no clock, no file system.
- A port is faithful first. A bug found in 1.1.1 is ported as it is, gets a `J` row in `parity.tsv`, and
  is fixed in a separate commit after the exact port passes.
- Fixtures use synthetic geometry and names (`SYN1000`, `DISPLAY1`..). Port the 14 tests marked
  `renamed-in-22665ee` from `git show 22665ee:tests/Tests.cs`. Never put a real desk layout, a real
  monitor name or a trace of real cursor positions into the repository.
- Dependencies: only crates already in the local cargo cache (`--offline` must work). Today that is
  serde, serde_json, windows-sys 0.61, eframe/egui 0.36 and the approved embed-resource 3.0.12 build dependency.
- No `rust-toolchain.toml` (rustup would download a toolchain). The workspace builds with rustc 1.98.

## Test names

- `pNNN_<words>`: proves parity row `PNNN` (one row may have several tests with the same prefix).
- `bNNN_`, `jNN_`: prove the `B` and `J` rows.
- `s_<crate>_<words>`: skeleton and support tests that prove no checklist row.
- Reference tests are `#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]` and FAIL, never skip, when the
  reference is missing, so a parity run cannot pass with zero reference cases.

## Test safety (the owner uses MouseFence 1.x all day)

- No test installs a real mouse hook, calls `SetCursorPos` or `ClipCursor`, or sends input.
- Hotkey tests register only chords on F24 and fire them with a posted `WM_HOTKEY` to our own window.
- Single-instance tests use a test-only mutex name, never `MouseFence_SingleInstance_2C7A`.
- Autostart tests write a throwaway key under `HKCU\Software\MouseFenceTest-<pid>`, never the Run key.
- Settings tests use temporary folders, never `%APPDATA%\MouseFence`.
- No test changes display settings. No test stops or replaces the running MouseFence.exe.
- Anything that opens a window (screenshots, the identify overlay) runs only through low_run, which
  starts it on the invisible desktop.

## Commands

Every build and test goes through low_run (BelowNormal priority, no window, invisible desktop): `<low_run>`
below stands for `python <path to low_run.py>` as the brief gives it. Run them
from any shell; `<wt>` is the worktree folder, `<log>` a file under your report folder.

```
<low_run> --cwd <wt> --log <log> -- cargo.exe test --workspace --offline
<low_run> --cwd <wt> --log <log> -- cargo.exe clippy --workspace --offline --all-targets -- -D warnings
<low_run> --cwd <wt> --log <log> -- cargo.exe fmt --all --check
```

The parity reference (C# 1.1.1 engine and settings code behind a JSON-lines interface):

```
<low_run> --cwd <wt> --log <log> -- dotnet.exe build tools/reference-cs/MouseFence.Reference.csproj -c Release -o <scratch>/reference
MF_REFERENCE_EXE=<scratch>/reference/MouseFence.Reference.exe <low_run> --cwd <wt> --log <log> -- cargo.exe test -p fence-trace --offline -- --ignored
```

The settings-document reference test (`config_reference_diff.rs`) also compares the seal answers, which need
Track A's compiler. While `compile` is the skeleton stub it FAILS; Track B's own gate before the join
adds `MF_SEALS_PENDING=1` in front of the command above as an explicit opt-out. The join gate must not
set it (with the compiler in, the opt-out fails the test) and must show the line
`sealed: compared=<SEAL_ANSWERS> mismatches=0 pending=0`.

Since the join (2026-10-13) the reference commands above are one: `cargo.exe test -p fence-engine -p fence-trace
--offline -- --ignored --nocapture` with `MF_REFERENCE_EXE` set runs the engine and the settings diffs. Each
seal answer is compared twice: the exact 1.1.1 validator (`Fixes::EXACT_1_1_1`) must equal the reference, and
2.0 may differ only where J05 fired (the line also prints `2.0-differs-by-J05=<n>`).

The settings window's tests use a fake backend by default; `MF_UI_REAL_BACKEND=1` in front of
`cargo.exe test -p fence-settings-ui --offline` runs every one of them against the real fence-config and
fence-engine. Expected differences are J05's only: P004 and P005 expect "needs a firm push" under the real
backend and "never crosses" under the fake, so the real-backend run is a green gate (fix round 2026-10-13).
`--screenshot` (not the demo) reads the settings with `fence_config::load_read_only`, which never writes.

## Shared rules, one implementation (join 2026-10-13)

- .NET string rules (Ordinal, OrdinalIgnoreCase, the `\\.\DISPLAY` prefix): `fence_engine::layout`, proven by the
  whole-plane sweep; `fence_engine::ids` re-exports them.
- `SanitizeExcluded`: `fence_engine::layout::sanitize_excluded`. `ValidateNoSeal`: `fence_config`'s
  `Settings::validate_no_seal[_with]`, used by the save path and the settings window alike.
- A judged fix is switchable through `fence_engine::fixes::Fixes`; a function whose answer a fix changes has a
  `_with(.., fixes) -> (answer, hit_bits)` form beside the plain one (which uses `Fixes::ALL`).
- Win32 calls that only the settings window makes (overlays, file dialogs, message boxes, theme reads) live in
  `crates/fence-settings-ui/src/win.rs`; every other Win32 call is in `fence-win` (JUDGE-D R5).

The done gate of a track (fails while any row of the track lacks its test):

```
MF_PARITY_STRICT=1 MF_PARITY_TRACK=A <low_run> --cwd <wt> --log <log> -- cargo.exe test -p parity --offline
```

The 1.1.1 suite stays green (it is the reference):

```
<low_run> --cwd <wt> --log <log> -- dotnet.exe run --project tests/MouseFence.Tests.csproj -c Release
```

low_run exit 75 means deferred (a game was running), 76 blocked (no invisible desktop): report them as
such, never as a pass or a failure, and never retry with `--no-hidden-desktop`.

## Commits

Commit on your own branch after every green test set, path-limited to the files you own. Never push,
never merge into main: the repository is public and both wait for the owner's yes.
