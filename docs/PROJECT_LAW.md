# MouseFence — project law (on demand; not auto-loaded)
- Scope and product decisions that bind this repo, one per line, each with its date and source.
- Folded on 2026-09-25 from `.claude/PROJECT_RULES.md` (rules dated 2026-08-19) and `docs/MOUSEFENCE_REFERENCE.md` (2026-09-10); verbatim originals: `F:/_archive/repo-staging/mousefence-rules-2026-09-25.tar`.

## Invariants (do NOT break) — PROJECT_RULES.md:3-8
1. **Barrier logic lives in the pure, Win32-free `GuardCore`** and stays unit-tested (`tests/Tests.cs`). Add/update a scenario for every behavior change.
2. **Never mutate the display configuration** (`ChangeDisplaySettingsEx` etc.). A "center monitor" feature was cut because the OS mode-set froze the machine. The app only watches the mouse.
3. **All UI text goes through `Strings`**, and **EN + TR dictionaries must have identical keys** (a test enforces parity → Turkish = fully Turkish, English = fully English). No hardcoded user-facing strings.
4. Keep it **light** — no NuGet deps, pure WinForms. Dark theme: manual `Theming` + owner-drawn tabs; combo dropdowns/MessageBox staying light is accepted.
5. The low-level hook ignores **injected** input → can't be tested with synthetic mouse events; verify live feel by hand.

## Process — PROJECT_RULES.md:11-13
- Design decisions → real council: separate executor families answer independently, then a cross-family critic; "council" is not Claude sub-agents alone. Pick the seats from `C:/Users/ydbil/.claude/estate/ROUTING_CARD.md` (2026-09-25: the old pointer `EXECUTOR_ROUTING_TABLES.md` no longer exists).
- The running `MouseFence.exe` under `%LOCALAPPDATA%\Programs\MouseFence\` is the owner's live tool: leave it alone; a redeploy stops only its PID, never by image name (STATUS history 2026-07-24; refactor brief 2026-07-24 :1,:79-81).

## Commands — MOUSEFENCE_REFERENCE.md:8-17 and STATUS history 2026-07-24 (:6, :36-42)
- Test: `dotnet run --project tests/MouseFence.Tests.csproj -c Release` (must stay green). It is a console harness; `dotnet test` is a no-op.
- Build: `dotnet build MouseFence.csproj -c Release --no-incremental -o F:/_scratch/<job-id>/build` (a running app locks `bin/Release`; `-o` sidesteps it; the old `-o STAGING/_build/...` target is retired with STAGING).
- Run from source: `dotnet run -c Release`. A running instance holds the single-instance mutex, so a second tray app exits at once; UI checks use `--screenshot-demo`, which runs before the mutex check.
- Dev-only screenshots: `MouseFence.exe --screenshot <png> [Light|Dark] [tabIndex] [en|tr]` or `--screenshot-demo <png> <Light|Dark> <tab 0/1/2/3, -1 boundary, -2 advanced-fold> <en|tr> [scale] [WxH]` (synthetic layout, no real hardware exposed; `Program.cs:16`).
- Publish, small exe: `dotnet publish -c Release -r win-x64 --self-contained false -p:PublishSingleFile=true`.
- Publish, standalone exe: `dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:EnableCompressionInSingleFile=true`.
- Release (goes outward, asks the owner): `gh release create vX.Y.Z <exe> --repo ydbilgin/MouseFence --title "..." --notes "..."`.
- Deploy: the live autostart copy is `%LOCALAPPDATA%\Programs\MouseFence\` (multi-file framework-dependent: exe + dll + deps/runtimeconfig json; the HKCU `Run` entry points there). Redeploy = `dotnet publish -c Release -r win-x64 --self-contained false -o <scratch>`, stop the running PID, copy the files over. The older `app/MouseFence.exe` path is stale.
- Settings file: `%APPDATA%\MouseFence\settings.json`.
