# MouseFence — AGENTS.md (the body for Claude and Codex; cx reads only a capped head, keep Purpose/Run/Never on top)
Purpose: MouseFence, a lightweight Windows tray utility (C# .NET 9, WinForms) that controls how the mouse cursor crosses edges in multi-monitor layouts.
Run: test `dotnet run --project tests/MouseFence.Tests.csproj -c Release` · build `dotnet build MouseFence.csproj -c Release --no-incremental -o F:/_scratch/<job-id>/build` · lint TODO(owner) (commands and deploy: docs/PROJECT_LAW.md "Commands")
Never: mutate the display configuration, move barrier logic out of the pure Win32-free GuardCore, or stop the owner's running MouseFence.exe except by its PID for a redeploy.
Truth: `STATUS.md` (RESUME + Waiting on owner). A STATUS line is a claim; verify it against the code.
Law: `docs/PROJECT_LAW.md` (open before a change that touches its topic). Specs from the brain: `docs/specs/`.
Outputs: dispatch reports go to `F:/_outputs/MouseFence/<YYYY-MM>/`, scratch to `F:/_scratch/<job-id>/`; never a `STAGING/` folder here.
Brain bridge: decisions and ideas live in the brain (`F:/Beyin`); a finished task leaves a summary of at most fifteen lines in `F:/Beyin/_intake/summaries/`.

## Project rows (admitted facts; one line each, id in brackets)
