<p align="center">
  <img src="docs/icon.png" alt="MouseFence" width="120">
</p>

<h1 align="center">MouseFence 🛡️🖱️</h1>

<p align="center">
  A tiny Windows tray app that stops your mouse cursor from drifting into monitors you don't want it on —<br>
  with per‑screen rules, a hotkey, and a "game mode" that confines the cursor to one screen.
</p>

<p align="center">
  <a href="https://github.com/ydbilgin/MouseFence/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/ydbilgin/MouseFence?label=release&color=2ea44f"></a>
  <a href="https://github.com/ydbilgin/MouseFence/releases"><img alt="Downloads" src="https://img.shields.io/github/downloads/ydbilgin/MouseFence/total?color=success"></a>
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-0078D6?logo=windows&logoColor=white">
  <img alt=".NET" src="https://img.shields.io/badge/.NET-9.0-512BD4?logo=dotnet&logoColor=white">
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/github/license/ydbilgin/MouseFence?color=blue"></a>
</p>

<p align="center">
  <img src="docs/settings.png" alt="MouseFence settings — General" width="430">
  <img src="docs/settings-monitors.png" alt="MouseFence settings — Monitors / crossing rules" width="430">
</p>

---

## The problem

On a multi‑monitor setup the cursor slips onto screens you didn't mean to reach — a display stacked *above* your main one, a side monitor you only use sometimes, or it escapes the game you're playing. MouseFence puts an invisible, configurable barrier so the cursor stays where you expect it, and crosses only when you actually mean to.

## Features

- 🚧 **One‑way "up" barrier** — the cursor can't drift up into a monitor mounted above the main row by accident.
- 🎛️ **Per‑screen crossing rules** — choose exactly which bottom screen may cross up into which top screen (great for layouts with several monitors above). Empty = the primary may cross up into every top.
- 🔒 **Side screens stay put** — a screen with no rule can never cross up; the empty "void" corners above side screens stop trapping the cursor.
- ↔️ **Side containment** — the same soft barrier on your main screen's **left/right** edges, working **both ways**: the cursor won't drift onto a side monitor *or* slip back onto your main screen by accident, but a deliberate sideways push always crosses. Tune its **strength** in Settings (how hard you must flick to break through). Toggle with **Ctrl + Alt + →**; on by default.
- 🎯 **Deliberate crossing (optional)** — by default a crossing takes an intentional push up **or** sideways (fast slides and jitter never leak through); turn it off in Settings to let any move across a barrier cross like a normal monitor edge.
- 🎮 **Game mode** — a hotkey that confines the cursor to whichever monitor it's on (so it can't escape a windowed/borderless game).
- 🔦 **Identify screens** — a button in Settings flashes a big number on each monitor (★ on the primary) so you can tell which physical screen is which while setting up rules.
- ⌨️ **Global hotkeys** — toggle up‑crossing (default **Ctrl + Alt + ↑**), side containment (default **Ctrl + Alt + →**) and game mode (default **Ctrl + Alt + G**); plus a tray double‑click and a **Pause** (default **Ctrl + Alt + P**).
- 🌗 **Light / Dark / Follow‑system theme** and **English / Türkçe** (auto‑detected from your OS).
- 🪶 **Safe & light** — it only watches the mouse; it **never changes your display arrangement**. DPI‑aware, single instance, no installer.

## How it works

MouseFence installs a **low‑level mouse hook** (`WH_MOUSE_LL`) and enforces a barrier line at the top edge of your bottom row of monitors. While the cursor is on the bottom row it can't go above that line — which blocks the top monitors *and* the empty void areas above side screens. Crossing up is allowed only through a **gate**: an X‑range opening (derived from your per‑screen rules) that a deliberate upward push, starting in that same opening, may pass. **Side containment** is the horizontal mirror of this: a soft barrier on the main screen's left/right edges that blocks accidental drift onto a side monitor while letting a deliberate sideways push through (returning from a side screen is always free, so the cursor can't get trapped). **Game mode** instead confines the cursor to the rectangle of the monitor it's currently on.

The barrier decision logic lives in a pure, Win32‑free class (`GuardCore`) with a deterministic test suite, so the tricky cases are verified without a multi‑monitor rig.

## Install

### Option A — download a release
1. Grab the latest **`MouseFence-win-x64.exe`** from the **[Releases page](https://github.com/ydbilgin/MouseFence/releases/latest)**.
2. Run it — it lives in the system tray, no installer, no window on launch.

> A framework‑dependent build needs the **.NET 9 Desktop Runtime**; a self‑contained build runs without it.

### Option B — build from source
You'll need the **.NET 9 SDK**.

```bash
git clone https://github.com/ydbilgin/MouseFence.git
cd MouseFence
dotnet run -c Release

# single-file exe (needs the .NET 9 runtime on the target machine)
dotnet publish -c Release -r win-x64 --self-contained false -p:PublishSingleFile=true
# or self-contained (bundles the runtime)
dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true
```

## Usage

- The tray icon shows state: 🔴 crossing closed · 🟢 crossing open · ⚪ paused.
- **Toggle up‑crossing** with the hotkey (default Ctrl + Alt + ↑) or a double‑click on the tray icon.
- **Side containment**: press its hotkey (default Ctrl + Alt + →) to switch the sideways soft barrier on/off.
- **Game mode**: press its hotkey (default Ctrl + Alt + G) to confine the cursor to its monitor; press again to release.
- **Right‑click the tray** for: toggle up‑crossing, side containment, game mode, pause/resume, Settings, Exit.
- **Crossing a barrier** (where allowed): aim into the middle of the screen and push **straight** across — up, or sideways for the side barrier.

## Settings

Open **Settings…** from the tray. Three tabs:

- **General** — the up‑crossing, side‑containment, game‑mode and pause hotkeys; start states (up‑crossing closed, side containment on); the **side barrier strength** (how hard you must flick to cross sideways); deliberate‑crossing toggle; start with Windows; and an **Identify** button that flashes a number on each screen.
- **Appearance** — language (Automatic / English / Türkçe) and theme (Follow system / Light / Dark).
- **Monitors** — which monitors count as "top" (auto‑detect or manual), and the **crossing rules**: pick a bottom screen, then check which top screens it may cross up into. A read‑only mini diagram shows your layout.

Settings persist to `%APPDATA%\MouseFence\settings.json` (plain JSON; delete it to reset).

## Troubleshooting / FAQ

**The cursor still slips between displays.** Turn off Windows' **Settings → System → Display → Multiple displays → "Ease cursor movement between displays."**

**It doesn't work in some games.** Low‑level mouse hooks can't act over **exclusive‑fullscreen** games or **administrator** windows (run MouseFence elevated for the latter). Windowed/borderless games are fine — that's what game mode is for.

**I can't cross up even when it's allowed.** The crossing is deliberate: push **straight up** from the middle of the screen. Make sure crossing is open (green icon) and the rule allows that bottom→top pair.

**My hotkey rotates the screen (Intel laptops).** `Ctrl + Alt + Arrow` is the Intel graphics driver's screen‑rotation shortcut. On a fresh install MouseFence detects an Intel GPU and adds **Shift** to its arrow hotkeys automatically; you can also pick any other combo in Settings.

**Mixed‑DPI monitors?** Handled — MouseFence is PerMonitorV2‑aware and works in physical pixels.

## Testing

The pure barrier logic (`GuardCore`) is covered by a deterministic scenario suite:

```bash
dotnet run --project tests\MouseFence.Tests.csproj -c Release
```

It covers side‑screen voids, origin‑aware diagonals, per‑screen multi‑gate rules, deliberate‑push thresholds, free roam + descent, and game‑mode confinement.

> The live cursor *feel* must still be verified by hand — the hook ignores injected/synthetic input, so it can't be exercised with simulated mouse events.

## Contributing

Contributions welcome. Keep the barrier logic in the Win32‑free `GuardCore` (so it stays testable), add/​update a scenario in `tests\Tests.cs`, and verify the feel by hand on a multi‑monitor setup before opening a PR.

## License

[MIT](LICENSE)
