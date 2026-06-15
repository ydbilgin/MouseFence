# MouseFence 🛡️🖱️

**A tiny Windows tray app that stops your mouse cursor from drifting up into a monitor mounted *above* your main screen — with a one‑key toggle for when you actually want to go there.**

> _[screenshot / gif placeholder — drop a short clip of the cursor hitting the invisible barrier and the tray icon flipping from red to green here]_

---

## The problem

If you have a monitor stacked **above** your primary display (or side monitors that leave empty "void" corners above them), Windows lets the cursor slide straight up there by accident. You overshoot a window's title bar, fling the pointer at a top‑edge menu, or slide sideways too fast — and suddenly the cursor is lost on the top screen, or worse, pinned against an invisible wall in a dead zone above a side monitor.

MouseFence puts up a smart, one‑way barrier so the cursor stays where you expect it — and lets you cross up **only when you mean to**.

## Features

- 🚧 **One‑way barrier** along the bottom edge of your top monitor(s). The cursor can't drift up by accident.
- 🔒 **Side screens are always protected** — you can never wander up into the top monitor from a side display, and the empty "void" areas above side screens stop trapping the cursor.
- 🎯 **Deliberate crossing from your main screen** — opt in with a global hotkey, then make a clear upward push to go up. Fast horizontal slides and tiny jitter never leak through.
- ⬆️ **Free roam once you're up there** — on the top monitor the cursor moves normally, and coming back down is never blocked.
- ⌨️ **Global hotkey** to open/close the gate (default **Ctrl + Alt + Up**), or just double‑click the tray icon.
- ⏸️ **Pause** to suspend the whole barrier instantly.
- 🚦 **Tray icon shows state** at a glance: red (blocked), green (open), grey (paused).
- ⚙️ **Simple settings GUI** — pick the hotkey, choose which monitors to block (auto‑detect or manual), set the startup state, and start with Windows.
- 🖥️ **DPI‑aware (PerMonitorV2)** — handles mixed‑DPI, multi‑monitor layouts.
- 🪶 **Single instance**, lightweight, no background services.

## How it works

MouseFence installs a **low‑level mouse hook** (`WH_MOUSE_LL`) and enforces a horizontal barrier line at the **bottom edge of the top monitor(s)** — which is the top edge of your primary screen (Y = 0). While the cursor is on the bottom row, it cannot go above that line. This single line also covers the empty void areas above side screens.

The crossing rule is deliberately strict:

- **Side screens:** crossing up is **never** allowed. Permanent.
- **Main screen:** crossing up is allowed only when the **gate is open** *and* you make a deliberate upward push inside an inset band of the main screen. The decision is **origin‑aware** — the move must both *start* and *end* inside the main gate column — so a fast diagonal coming off a side screen can't sneak up just because it happens to land in the gate column.
- **Once above the line,** the cursor roams the whole top monitor freely and can always descend back down.

Blocked moves are clamped to the barrier line (keeping your X so horizontal sliding stays smooth). The hook ignores injected/synthetic input, so its own corrections never recurse.

The core decision logic lives in a small, pure class (`GuardCore`) with **no Win32 dependencies**, which is why it can be unit‑tested deterministically (see [Testing](#testing)).

## Install

### Option A — Download a release

1. Grab the latest `MouseFence.exe` from the **[Releases page](https://github.com/ydbilgin/MouseFence/releases)**.
2. Run it. It lives in the system tray — there's no installer and no window on launch.

> The standard release needs the **.NET 9 Desktop Runtime**. If you don't have it, use a self‑contained build (see below) that bundles the runtime.

### Option B — Build from source

You'll need the **.NET 9 SDK**.

```bash
git clone https://github.com/ydbilgin/MouseFence.git
cd MouseFence

# Run directly from source
dotnet run -c Release

# Build a single-file exe (requires the .NET 9 runtime on the target machine)
dotnet publish -c Release -r win-x64 --self-contained false -p:PublishSingleFile=true

# Or a self-contained build that bundles the runtime (for machines without .NET installed)
dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true
```

The published `MouseFence.exe` lands under `bin\Release\net9.0-windows\win-x64\publish\`.

## Usage

- **Launch** MouseFence — it appears in the system tray. The barrier is active immediately.
- **Tray icon color** tells you the state:
  - 🔴 **Red** — main → top crossing is *blocked* (gate closed)
  - 🟢 **Green** — main → top crossing is *open* (gate open)
  - ⚪ **Grey** — the tool is *paused*
- **Toggle the gate** with the global hotkey (default **Ctrl + Alt + Up**) or by **double‑clicking** the tray icon. A short balloon tip confirms the new state.
- **Right‑click the tray icon** for the menu: toggle the gate, pause/resume the tool, open Settings, or quit.
- **Crossing up from the main screen** (when the gate is open): aim into the middle band of the screen and push **straight up**. A small jitter or a fast sideways slide won't carry you across — that's by design.
- **Pause** suspends the entire barrier (the hook is removed) until you resume.

## Configuration

Open **Settings…** from the tray menu. Options:

- **Toggle hotkey** — click the box, then press your combo (needs at least one modifier + a key). Default: `Ctrl + Alt + Up`.
- **Which monitor(s) to block:**
  - *Auto* — automatically block any monitor sitting entirely above the primary.
  - *Manual* — tick the specific monitors from the list.
- **Start with the gate closed** — whether main → top crossing begins blocked on launch.
- **Start with Windows** — adds/removes a per‑user startup entry (`HKCU\…\Run`).

Settings persist to:

```
%APPDATA%\MouseFence\settings.json
```

It's plain JSON — safe to inspect, back up, or delete (deleting it just restores defaults).

## Troubleshooting / FAQ

**The cursor still slips between displays.**
Windows has its own setting that can fight MouseFence. Go to **Settings → System → Display → Multiple displays** and turn off **"Ease cursor movement between displays."**

**The barrier doesn't work over certain windows.**
Low‑level mouse hooks **can't intercept input over elevated/administrator windows** unless MouseFence itself runs elevated, and they **don't apply inside exclusive‑fullscreen games**. This is a Windows limitation, not a bug.

**I can't cross up even with the gate open.**
The crossing is intentionally deliberate: push **straight up** from the **middle band** of the main screen (the gate is inset from the screen edges). Fast diagonals and tiny jitters are rejected on purpose. Make sure the gate is open (green icon) and that the move starts on the main screen.

**The hotkey didn't register.**
Another app may already own that combo — MouseFence shows a balloon tip if registration fails. Pick a different combination in Settings.

**Mixed‑DPI monitors?**
Handled. MouseFence is PerMonitorV2‑aware and works in physical pixels across displays with different scaling.

> ℹ️ The tray menu and settings window are currently in **Turkish**. English/other‑language localization is welcome — see Contributing.

## Testing

The barrier decision logic is isolated in the pure, Win32‑free `GuardCore` class and backed by a deterministic scenario test suite:

```bash
dotnet run --project tests\MouseFence.Tests.csproj -c Release
```

The suite covers the tricky cases — side‑screen voids, origin‑aware diagonals from side screens, gate open/closed, jitter rejection, free roam after crossing, and descent — and exits non‑zero on any failure.

> **Note:** the live cursor *feel* must still be verified by hand. The hook deliberately ignores injected/synthetic input, so it cannot be exercised with simulated mouse events — only by moving a real mouse on real hardware.

## Contributing

Contributions are welcome. If you're changing barrier behavior, please add or update a scenario in `tests\Tests.cs` and make sure the suite passes. Keep the Win32‑free logic in `GuardCore` so it stays testable, and verify the cursor feel by hand on a multi‑monitor setup before opening a PR.

## License

[MIT](LICENSE)
