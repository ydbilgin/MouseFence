# Security policy

## Supported versions

Only the [latest release](https://github.com/ydbilgin/MouseFence/releases/latest) receives fixes.

## Reporting a vulnerability

Please report security problems privately through GitHub's private vulnerability reporting:
<https://github.com/ydbilgin/MouseFence/security/advisories/new>. Do not open a public issue for them.
Include the MouseFence version, your Windows version, and steps to reproduce.

## What MouseFence touches

- **Mouse input:** a low-level mouse hook (`WH_MOUSE_LL`) sees pointer movement system-wide and may move the
  pointer back to an edge (`SetCursorPos`). It does not record or store movement.
- **Files:** `%APPDATA%\MouseFence\settings.json` and a size-limited `%APPDATA%\MouseFence\error.log`, plus any
  backup file you choose to export.
- **Registry:** only when you turn on *Start with Windows*: one value named `MouseFence` under
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, removed again when you turn it off.
- **Global shortcuts:** registered with `RegisterHotKey`.
- **Network:** none. MouseFence has no network code, telemetry or update check.
- It never changes the Windows display configuration and needs no administrator rights.
