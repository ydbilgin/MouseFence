# Shot list: real recordings for the README

The README hero animation today is `docs/demo.gif`, an **illustration** rendered from `docs/demo/scene.html` and captioned
as such. The Settings pictures in `docs/screenshots/` are real captures from the app's `--screenshot-demo` mode.
What is still missing is a **real screen recording** of the pointer meeting a fence. The scenes below are the
ones to record on Windows. Each has a placeholder (`*.placeholder.svg`) in this folder that shows the target
size and file name.

## General setup (all scenes)

- Recorder: any region recorder that writes GIF or MP4 (for example ScreenToGif or ShareX), 30 fps.
- Display scaling **100%** on the recorded screens, so the GIF is sharp and the pointer is a normal size.
- Pointer **visible**, default Windows pointer, normal size.
- **Dark** Windows theme and MouseFence **Theme: Dark** (matches the README's dark captures).
- Build from current `main` (1.2.0), since the README describes `main`.
- Privacy: hide the taskbar (auto-hide) or crop it out; plain wallpaper; close chats, mail and browser tabs
  with personal names; do not show your real screen names in Settings (use Main / Top / Laptop).
- Keep each GIF under about 3 MB (GitHub shows it quickly); trim dead frames at the start and end.

## Scene 1: a firm push crosses, a slow drift stops

- File: `docs/media/firm-push.gif` (placeholder: `firm-push.placeholder.svg`)
- Capture: a **1280x720** region that straddles the edge between your main screen and the screen above it
  (about 2/3 main screen, 1/3 top screen).
- Setup: Settings → Displays → gate between Main and Top → **Needs a firm push**, **Save**. Upward passage
  open (tray double-click). Side protection as you like.
- Steps:
  1. (0–3 s) Drift the pointer slowly up to the edge, along the edge for a second; it stays on the main screen.
  2. (3–5 s) Aim for something near the top of the main screen (a browser tab strip); overshoot slightly.
     The pointer stops at the edge.
  3. (5–7 s) Move back down a little, then one quick straight push up: the pointer crosses onto the top screen.
  4. (7–9 s) Move back down: it crosses freely.
- Duration: about 9 s, loop.

## Scene 2: setting an edge in Settings

- File: `docs/media/set-an-edge.gif` (placeholder: `set-an-edge.placeholder.svg`)
- Capture: **1280x720** region around the Settings window (window size about 1064x820 at 100%; crop
  evenly).
- Steps:
  1. (0–2 s) Right-click the tray icon, choose **Settings...**; Displays opens.
  2. (2–5 s) Click the gate between Main and Top; the edge editor opens.
  3. (5–8 s) Choose **Crosses freely**, then **Needs a firm push**; open **Set each direction separately**
     to show that coming down stays free.
  4. (8–10 s) Select **Save**.
- Duration: about 10 s, loop.

## Optional scene 3: Keep pointer off

- File: `docs/media/keep-pointer-off.gif` (no placeholder; record only if Scene 1 is done)
- Capture: **1280x720** region across the edge to a side screen.
- Steps: tick **Keep pointer off** for the side screen in Displays, save, then move fast toward it several
  times; the pointer never enters. Duration about 6 s.

## Swapping the recordings into the README

- Scene 1 becomes the hero. The hero is generated: in `docs/showcase/showcase.yml` set `hero.media` to
  `docs/media/firm-push.gif`, update `hero.alt`, and replace `hero.caption` ("Illustration of the behaviour …")
  with a short one such as "Recorded on Windows 11 at 100% scaling". Then run
  `python3 docs/showcase/showcase.py` (see [docs/showcase/UPDATING.md](../showcase/UPDATING.md)). Keep
  `docs/demo.gif` and link it from [docs/demo/README-demo.md](../demo/README-demo.md) if you still want it.
- Scene 2 goes under **Everyday use**, after the three setup steps, at `width="720"`; it can replace the
  static edge-editor picture there.
- Delete the matching `*.placeholder.svg` once its recording is in.
