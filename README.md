<p align="center">
  <img src="docs/icon.png" alt="MouseFence shield icon" width="112">
</p>

<h1 align="center">MouseFence</h1>

<p align="center">
  <strong>Keep the mouse pointer on the screen you meant to use.</strong><br>
  A lightweight Windows tray utility for controlling how the cursor crosses edges in multi-monitor layouts.
</p>

<p align="center">
  <a href="https://github.com/ydbilgin/MouseFence/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/ydbilgin/MouseFence?label=release&color=2ea44f"></a>
  <a href="https://github.com/ydbilgin/MouseFence/releases"><img alt="Total downloads" src="https://img.shields.io/github/downloads/ydbilgin/MouseFence/total?color=success"></a>
  <img alt="Windows 10 and 11" src="https://img.shields.io/badge/Windows-10%20%7C%2011-0078D6?logo=windows&logoColor=white">
  <img alt=".NET 9" src="https://img.shields.io/badge/.NET-9.0-512BD4?logo=dotnet&logoColor=white">
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/github/license/ydbilgin/MouseFence?color=blue"></a>
</p>

<p align="center">
  <a href="https://github.com/ydbilgin/MouseFence/releases/latest"><strong>Download the latest release</strong></a>
  &nbsp;·&nbsp;
  <a href="#quick-start">Quick start</a>
  &nbsp;·&nbsp;
  <a href="#build-and-test">Build and test</a>
</p>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/boundary-editor-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="docs/screenshots/boundary-editor-light.png">
  <img src="docs/screenshots/boundary-editor-light.png" alt="MouseFence boundary editor using a synthetic Main, Top, and Laptop display layout" width="100%">
</picture>

<p align="center"><sub>The documentation uses a synthetic three-screen layout. It does not depict the maintainer's monitors.</sub></p>

## Stop cursor slips without fighting your display layout

A display above your main screen creates an easy escape route whenever you aim near the top edge. A side display can pull the pointer out of a borderless game, or catch it while you are working quickly. MouseFence adds configurable pointer boundaries while leaving the Windows display arrangement untouched.

Click a gate between two screens and choose the result:

| Boundary behavior | What it feels like |
|---|---|
| **Crosses freely** | The edge behaves like a normal Windows monitor edge. |
| **Needs a firm push** | Slow drift stops at the edge; one deliberate push crosses. |
| **Never crosses** | The pointer stays on its current side of that boundary. |

Side boundaries can also use different behavior in each direction. If a display should not receive the pointer at all, mark that screen **Keep pointer off**.

## What MouseFence includes

- **Visual boundary editing:** a read-only map mirrors the Windows layout and exposes clickable gates where screens meet.
- **Upward and side protection:** stop accidental movement onto screens above, left, or right of the main display.
- **Per-boundary rules:** use free, firm-push, or blocked behavior, with overall defaults in Advanced settings.
- **Keep-pointer-off screens:** exclude an attached display without disabling or rearranging it in Windows.
- **Game mode:** confine the pointer to whichever monitor it is currently on, useful for windowed and borderless games.
- **Safety escapes:** layout changes cannot silently leave the pointer sealed onto a screen when safety is enabled.
- **Global shortcuts:** toggle upward passage, side protection, game mode, or pause MouseFence from anywhere.
- **Native settings:** Light, Dark, and system themes; English and Turkish; local backup and restore.
- **Small scope:** single-instance WinForms tray app, DPI-aware, no installer, and no display mode changes.

## The example layout

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/displays-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="docs/screenshots/displays-light.png">
  <img src="docs/screenshots/displays-light.png" alt="Synthetic MouseFence display map and screen list" width="100%">
</picture>

The screenshots use three neutral, deterministic screens:

| Screen | Synthetic geometry | Why it is shown |
|---|---:|---|
| **Main** | 2560 × 1440 | The primary work or game display. |
| **Top** | 1920 × 1080, centered above Main | Makes the upward gate and firm-push behavior easy to understand. |
| **Laptop** | 1920 × 1080, touching Main on the right | Demonstrates a side screen marked **Keep pointer off**. |

This arrangement explains both common use cases in one compact picture: deliberate access to an upper screen and complete exclusion of a side screen.

<details>
<summary><strong>See every Settings view in Light and Dark</strong></summary>

### Boundary editor

<p>
  <img src="docs/screenshots/boundary-editor-light.png" alt="Boundary editor in Light theme" width="49%">
  <img src="docs/screenshots/boundary-editor-dark.png" alt="Boundary editor in Dark theme" width="49%">
</p>

### Advanced

<p>
  <img src="docs/screenshots/advanced-light.png" alt="Advanced settings in Light theme" width="49%">
  <img src="docs/screenshots/advanced-dark.png" alt="Advanced settings in Dark theme" width="49%">
</p>

### Shortcuts

<p>
  <img src="docs/screenshots/shortcuts-light.png" alt="Shortcut settings in Light theme" width="49%">
  <img src="docs/screenshots/shortcuts-dark.png" alt="Shortcut settings in Dark theme" width="49%">
</p>

### General

<p>
  <img src="docs/screenshots/general-light.png" alt="General settings in Light theme" width="49%">
  <img src="docs/screenshots/general-dark.png" alt="General settings in Dark theme" width="49%">
</p>

### Turkish localization

<p align="center">
  <img src="docs/screenshots/boundary-editor-dark-tr.png" alt="Turkish boundary editor in Dark theme" width="80%">
</p>
</details>

## Quick start

### Download a release

1. Download **`MouseFence-win-x64.exe`** from the [latest release](https://github.com/ydbilgin/MouseFence/releases/latest).
2. Run it. MouseFence starts in the system tray and opens no window at launch.
3. Right-click the tray icon, choose **Settings**, then open **Displays**.
4. Optionally name the screens or mark a display **Keep pointer off**.
5. Click a small gate in the map, choose its crossing behavior, and select **Save**.

A framework-dependent package needs the [.NET 9 Desktop Runtime](https://dotnet.microsoft.com/download/dotnet/9.0). A self-contained package includes the runtime.

### Everyday controls

- Double-click the tray icon to toggle upward passage.
- Right-click the tray icon for upward passage, side protection, game mode, pause, Settings, and Exit.
- Typical shortcut defaults are **Ctrl + Alt + Up**, **Ctrl + Alt + Right**, **Ctrl + Alt + G**, and **Ctrl + Alt + P**.
- On a fresh Intel graphics system, MouseFence may add **Shift** to an arrow shortcut to avoid the driver's screen-rotation binding.
- To cross a firm-push boundary, move toward the middle of the shared edge and push straight across.

Settings are stored as plain JSON at `%APPDATA%\MouseFence\settings.json`. Delete the file to reset the app.

## How it works

MouseFence installs a low-level mouse hook (`WH_MOUSE_LL`) and observes physical pointer movement. `BoundaryGraph` turns the saved display rules into directed transitions between real screen rectangles. The Win32-free `GuardCore` then decides whether each move passes, stops at an edge, or lands on an allowed monitor. Game mode uses the same core to confine movement to one monitor.

The application reads display geometry and stable monitor identities. It never calls Windows APIs that rearrange displays or change display modes.

## Troubleshooting

**The pointer still slips between displays.**

Turn off Windows **Settings → System → Display → Multiple displays → Ease cursor movement between displays**.

**A firm-push gate does not cross.**

Check that upward passage or side protection is enabled, confirm the selected boundary allows crossing, and push straight through the shared edge rather than diagonally near a corner.

**MouseFence does not work over a game or administrator window.**

Low-level hooks cannot control exclusive-fullscreen games or a process running at a higher privilege level. Windowed and borderless games are the intended game-mode targets. Run MouseFence elevated only when it must interact with an elevated application.

**The pointer cannot enter a display.**

Open **Displays** and check whether that screen is marked **Keep pointer off**, or whether its boundary is set to **Never crosses**.

**Mixed-DPI monitors?**

MouseFence is PerMonitorV2-aware and uses physical pixels.

## Build and test

MouseFence requires Windows and the .NET 9 SDK.

```powershell
git clone https://github.com/ydbilgin/MouseFence.git
cd MouseFence
dotnet run -c Release
```

Create a framework-dependent single-file executable:

```powershell
dotnet publish -c Release -r win-x64 --self-contained false -p:PublishSingleFile=true
```

Or create a self-contained executable:

```powershell
dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:EnableCompressionInSingleFile=true
```

Run the deterministic scenario suite with `dotnet run`. The test project is a console harness, so `dotnet test` is not the test command for this repository.

```powershell
dotnet build MouseFence.csproj -c Release --no-incremental -o STAGING/_build/main
dotnet run --project tests/MouseFence.Tests.csproj -c Release
```

The suite covers boundary compilation, deliberate-push thresholds, side containment, void correction, per-direction rules, safety escapes, settings migration, localization parity, and the synthetic documentation capture.

### Reproduce the generic screenshots

The documentation harness uses only synthetic monitor geometry and fresh demo settings. It keeps the window beyond the virtual desktop, never activates it, and never reads pixels from the owner's screen.

```powershell
STAGING/_build/main/MouseFence.exe --screenshot-demo docs/screenshots/example.png Dark -1 en
```

Preview values are `0` Displays, `1` Advanced, `2` Shortcuts, `3` General, `-1` boundary editor, and `-2` Advanced with physical-pixel controls expanded. The final argument is `en` or `tr`.

## Contributing

Contributions are welcome. Keep pointer-decision behavior in the Win32-free `GuardCore`, add or update a deterministic scenario in `tests/Tests.cs`, run the console test harness, and verify live pointer feel on Windows when behavior changes.

The hook intentionally ignores injected input, so real cursor feel cannot be validated with simulated mouse events alone.

## License

[MIT](LICENSE)
