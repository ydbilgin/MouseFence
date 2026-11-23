# Security policy

## Supported versions

Only the [latest release](https://github.com/ydbilgin/MouseFence/releases/latest) receives fixes.

## Reporting a vulnerability

Please report security problems privately through GitHub's private vulnerability reporting:
<https://github.com/ydbilgin/MouseFence/security/advisories/new>. Do not open a public issue for them.
Include the MouseFence version, your Windows version, and steps to reproduce.

## What MouseFence touches

- **Mouse input:** a low-level mouse hook (`WH_MOUSE_LL`) sees pointer movement system-wide and may move the
  pointer back to an edge (`SetCursorPos`). It does not store movement unless you turn on *Record edge moves*
  (Advanced, off by default): then moves near the edges (pointer positions, raw mouse deltas, button and key
  state, monitor IDs) are written to `%LOCALAPPDATA%\MouseFence\v2\traces\*.jsonl`, at most 300 files and 20 MB,
  and never leave the PC. Monitor IDs are Windows device paths that name the monitor model; check a trace before
  you share it.
- **Files:** `%APPDATA%\MouseFence\v2\settings.json`, a size-limited `%APPDATA%\MouseFence\v2\error.log` and a small
  launch state file in `%LOCALAPPDATA%\MouseFence\v2\`, plus any backup file you choose to export. The error log
  can contain file paths with your Windows user name; remove it before posting a log in an issue.
- **Registry:** only when you turn on *Start with Windows*: one value named `MouseFence` under
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, removed again when you turn it off.
- **Global shortcuts:** registered with `RegisterHotKey`.
- **Network:** none. MouseFence has no network code, telemetry or update check.
- It never changes the Windows display configuration and needs no administrator rights.
