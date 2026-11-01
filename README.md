<p align="center">
  <img src="docs/icon.png" alt="MouseFence icon: a red fence bar above a mouse pointer" width="112">
</p>

<h1 align="center">MouseFence</h1>

<p align="center">
  <strong>Keep the mouse pointer on the screen you meant to use.</strong><br>
  A small Windows tray app that puts gentle fences on the edges between your monitors.
</p>

<p align="center">
  <a href="https://github.com/ydbilgin/MouseFence/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/ydbilgin/MouseFence?include_prereleases&sort=semver"></a>
  <img alt="Platform: Windows 10 and 11" src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D6">
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-green"></a>
</p>

<p align="center">
  <img src="docs/demo.gif" alt="Illustration: the pointer stops at the edge of the screen above, crosses with one firm push, and never enters a screen marked Keep pointer off" width="800"><br>
  <sub>Illustration of the behaviour, drawn from the app's own screenshots — not a screen recording.</sub>
</p>

<p align="center">
  <a href="https://github.com/ydbilgin/MouseFence/releases/latest"><strong>Download</strong></a>
  &nbsp;·&nbsp; <a href="#what-each-edge-can-do">What it does</a>
  &nbsp;·&nbsp; <a href="#install">Install</a>
  &nbsp;·&nbsp; <a href="#everyday-use">Use</a>
  &nbsp;·&nbsp; <a href="#build-from-source">Build</a>
  &nbsp;·&nbsp; <a href="#questions">FAQ</a>
  &nbsp;·&nbsp; <a href="#türkçe">Türkçe</a>
</p>

## The problem

With more than one monitor, the pointer slips onto the wrong screen all the time. A screen above your main
one catches it whenever you aim for a tab or a title bar near the top edge. A side screen pulls it out of a
borderless game in the middle of a fight. Windows lets you arrange your displays, but every shared edge is
always open.

MouseFence leaves your Windows display layout exactly as it is and only decides what happens when the
pointer reaches the edge between two screens.

## What each edge can do

Click the small gate between two screens and choose:

| Edge behaviour | What it feels like |
|---|---|
| **Crosses freely** | A normal Windows monitor edge. |
| **Needs a firm push** | Slow drift stops at the edge; one quick, deliberate move crosses. |
| **Never crosses** | The pointer stays on its side. |

Each direction is set on its own: going up into the top screen can need a firm push while coming back down
crosses freely. On top of the edges:

- **Keep pointer off** a screen: the pointer never enters it, and Windows keeps using it as normal.
- **Game mode** holds the pointer on whichever screen it is on (windowed and borderless games).
- **Upward passage** and **side protection** switches, plus **Pause**, each with a global shortcut and a
  tray menu entry.
- **Safety:** after a layout change MouseFence keeps a way out of every screen, so the pointer is never
  sealed in.
- Light, Dark or system theme, English and Turkish, settings backup and restore.

It is a single-instance tray app, per-monitor DPI-aware, with no installer and no network code, and it never
changes your display settings.

## Install

1. Download **`MouseFence-win-x64.exe`** from the [latest release](https://github.com/ydbilgin/MouseFence/releases/latest).
2. Install the [.NET 9 Desktop Runtime](https://dotnet.microsoft.com/download/dotnet/9.0) if you do not have
   it yet (the release file is small because it uses the shared runtime).
3. Put the file anywhere, for example `%LOCALAPPDATA%\Programs\MouseFence\`, and run it. It starts in the
   system tray and opens no window.
4. To start it with Windows, turn on **Settings → General → Start with Windows**.

> [!NOTE]
> This README and its pictures describe the code on `main` (version 1.2.0, not released yet). The latest
> release, v1.1.1, has the same fences with an older look; 1.2.0 measures a firm push by the speed of the
> current move and adds a **Crossing speed** setting. To try it today, [build from source](#build-from-source).

## Everyday use

1. Right-click the tray icon and choose **Settings**. **Displays** shows your screens as Windows arranges them.
2. Optionally name each screen, or tick **Keep pointer off** for a screen the pointer should never enter.
3. Click a small gate where two screens meet, pick **Crosses freely**, **Needs a firm push** or
   **Never crosses**, and select **Save**.

To cross a firm-push edge, move to the middle of the shared edge and push straight across in one quick
motion. If it feels too stiff or too loose, change **Advanced → Crossing speed (px/s)** (default 3000;
higher feels firmer).

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/boundary-editor-dark.png">
    <img src="docs/screenshots/boundary-editor-light.png" alt="Settings, Displays page: choosing what the edge between Main and Top does" width="720">
  </picture>
</p>

Double-click the tray icon to open or close upward passage; right-click it for the menu. Default shortcuts
(change them in **Settings → Shortcuts**):

| Action | Shortcut |
|---|---|
| Upward passage on or off | `Ctrl + Alt + Up` |
| Side protection on or off | `Ctrl + Alt + Right` |
| Game mode | `Ctrl + Alt + G` |
| Pause MouseFence | `Ctrl + Alt + P` |

On a fresh install on a PC with Intel graphics, MouseFence adds `Shift` to the two arrow shortcuts, because
the Intel driver uses `Ctrl + Alt + arrow` to rotate the screen.

<details>
<summary><strong>Every Settings page and the tray menu, in Light and Dark</strong></summary>

These are real captures of the app, drawn off-screen by its `--screenshot-demo` mode with a made-up
three-screen layout (Main 2560 × 1440, Top above it, Laptop to the right), not anyone's real monitors.

**Displays**

<p>
  <img src="docs/screenshots/displays-light.png" alt="Displays page in Light theme" width="49%">
  <img src="docs/screenshots/displays-dark.png" alt="Displays page in Dark theme" width="49%">
</p>

**Advanced**

<p>
  <img src="docs/screenshots/advanced-light.png" alt="Advanced settings in Light theme" width="49%">
  <img src="docs/screenshots/advanced-dark.png" alt="Advanced settings in Dark theme" width="49%">
</p>

**Shortcuts**

<p>
  <img src="docs/screenshots/shortcuts-light.png" alt="Shortcut settings in Light theme" width="49%">
  <img src="docs/screenshots/shortcuts-dark.png" alt="Shortcut settings in Dark theme" width="49%">
</p>

**General**

<p>
  <img src="docs/screenshots/general-light.png" alt="General settings in Light theme" width="49%">
  <img src="docs/screenshots/general-dark.png" alt="General settings in Dark theme" width="49%">
</p>

**Tray menu** and **Turkish**

<p>
  <img src="docs/screenshots/tray-menu-light.png" alt="Tray menu in Light theme: Upward passage, Side protection, Game mode, Pause, Settings, Exit, each with its shortcut" width="240">
  <img src="docs/screenshots/tray-menu-dark.png" alt="Tray menu in Dark theme" width="240">
</p>

<p>
  <img src="docs/screenshots/boundary-editor-dark-tr.png" alt="Turkish edge editor in Dark theme" width="80%">
</p>
</details>

## How it works

```mermaid
flowchart LR
  A[Physical mouse move] --> B["Low-level mouse hook<br/>Platform/MouseGuard.cs"]
  B --> C["GuardCore<br/>pure, Win32-free decision"]
  D["Your edge rules<br/>Settings.cs"] --> E["BoundaryGraph<br/>rules → directed edges"]
  F["Live monitor rectangles<br/>Core/MonitorInfo.cs"] --> E
  E --> C
  C -->|pass| G[Pointer moves on]
  C -->|block| H["Held at the edge<br/>SetCursorPos"]
```

`BoundaryGraph` turns your saved rules into directed edges between the real screen rectangles. For each
physical move, `GuardCore` decides whether it passes, stops at the edge, or lands on an allowed screen; a
firm-push edge looks only at the speed of that single move. MouseFence reads screen positions and stable
monitor identities and never calls the Windows functions that rearrange displays or change display modes.

## Questions

**The pointer still jumps between screens on its own.**
Turn off Windows **Settings → System → Display → Multiple displays → Ease cursor movement between displays**.
That Windows 11 setting moves the pointer across edges for you and works against the fences.

**Does it send anything over the network?**
No. MouseFence has no network code: no telemetry, no update check. It stores only `settings.json` (plain
JSON; delete it to reset) and a small, size-limited `error.log` in `%APPDATA%\MouseFence\`.

**It does nothing over a game or an administrator window.**
Windows does not let a normal app steer the pointer over exclusive-fullscreen games or over programs running
as administrator. Windowed and borderless games work. Run MouseFence as administrator only if you need it
over an elevated program.

**The pointer cannot reach one of my screens.**
Open **Displays**: that screen is probably marked **Keep pointer off**, or the edge to it is **Never crosses**.
Also check in the tray menu that MouseFence is not paused.

**It will not start and says the settings file is invalid.**
MouseFence never overwrites a settings file it cannot read. Restore a backup from **General → Restore from a
file**, or delete `%APPDATA%\MouseFence\settings.json` to start fresh. Details are in `error.log`.

**How do I uninstall it?**
Turn off **Start with Windows** (it removes MouseFence's entry from your user's startup list), exit from the
tray menu, then delete the exe and the `%APPDATA%\MouseFence\` folder.

## Build from source

You need Windows and the [.NET 9 SDK](https://dotnet.microsoft.com/download/dotnet/9.0).

```powershell
git clone https://github.com/ydbilgin/MouseFence.git
cd MouseFence
dotnet run -c Release
```

A single-file exe that uses the shared .NET runtime, like the release (add `--self-contained true
-p:EnableCompressionInSingleFile=true` for one that carries its own runtime):

```powershell
dotnet publish -c Release -r win-x64 --self-contained false -p:PublishSingleFile=true
```

**Tests.** The test project is a console program, so use `dotnet run`, not `dotnet test`:

```powershell
dotnet run --project tests/MouseFence.Tests.csproj -c Release
```

It covers edge rules, firm-push speed, side protection, safety escapes, settings migration, the
English/Turkish texts, and the screenshot pictures.

**Screenshots.** The pictures above come from the app itself, drawn off-screen with made-up screens; nothing
is read from your monitors:

```powershell
dotnet build MouseFence.csproj -c Release -o build
build/MouseFence.exe --screenshot-demo docs/screenshots/displays-dark.png Dark 0 en
build/MouseFence.exe --screenshot-tray docs/screenshots/tray-menu-dark.png Dark en
```

The view number is `0` Displays, `1` Advanced, `2` Shortcuts, `3` General, `-1` edge editor, and `-2`
Advanced with the physical-pixel controls open. The last argument is `en` or `tr`. The illustration at the
top is rendered from `docs/demo/scene.html`; see [docs/demo/README-demo.md](docs/demo/README-demo.md).
More notes for contributors are in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Contributing, security, license

Contributions are welcome; [CONTRIBUTING.md](CONTRIBUTING.md) explains how to build, test and send a change.
Please report security problems privately as described in [SECURITY.md](SECURITY.md).
MouseFence is released under the [MIT license](LICENSE).

## Türkçe

**MouseFence**, çok monitörlü Windows kurulumlarında fare imlecinin yanlış ekrana kaymasını önleyen küçük bir
sistem tepsisi uygulamasıdır. Ekran düzeninize dokunmaz; yalnızca iki ekranın birleştiği kenarda ne olacağına
karar verir: serbest geçiş, kararlı bir itişle geçiş ya da hiç geçmeme. Bir ekranı tamamen imleç dışı
bırakabilir, oyun modunda imleci bulunduğu ekrana kilitleyebilirsiniz. Arayüz Türkçe de kullanılabilir.

- **Kurulum:** [son sürümden](https://github.com/ydbilgin/MouseFence/releases/latest)
  `MouseFence-win-x64.exe` dosyasını indirin, gerekirse .NET 9 Desktop Runtime'ı kurun ve çalıştırın.
- **Kullanım:** tepsi simgesine sağ tıklayıp **Ayarlar**'ı açın, iki ekran arasındaki kapıya tıklayıp
  davranışı seçin ve kaydedin. Ayrıntılar: [Everyday use](#everyday-use), sorular: [Questions](#questions).
- Ağ bağlantısı kurmaz; yalnızca ayarlarını ve küçük bir hata günlüğünü `%APPDATA%\MouseFence\` içinde tutar.
