# The README animations

Three films, all drawn, none recorded. Each is a small web page that paints any moment of the film when asked
(`window.renderAt(seconds)`); `render.mjs` steps through it frame by frame in a headless Chromium and lets
ffmpeg turn the frames into an optimised GIF. Nobody's screen is captured.

| Film | Page | GIF | What it shows |
|---|---|---|---|
| `scene` | `scene.html` | `docs/demo.gif` | The overview: drift stops, a push crosses, a screen kept off, Settings and tray. |
| `edges` | `edges.html` | `docs/media/edges.gif` | One hand movement against the three edge behaviours side by side, with its upward speed. |
| `game-mode` | `game-mode.html` | `docs/media/game-mode.gif` | Game mode and its shortcut: a flick out of a borderless game, held, released. |

`edges.html` replays a small deterministic simulation of the README's rules for every frame: a free edge passes
everything, a firm-push edge passes only a fast enough upward move, a never-crosses edge passes nothing upward.
The speed threshold in it is a simplification for the picture, not the app's exact crossing rule.

## Re-render in one command

From the repository root:

```powershell
node docs/demo/render.mjs --sheet
node docs/demo/render.mjs edges --sheet
node docs/demo/render.mjs game-mode --sheet
```

Each rewrites its GIF and writes a contact sheet next to the pages (one frame per second, for a quick look;
git-ignored). Add `--mp4` for an MP4 beside the GIF as well.

Needs Node.js 18 or newer, ffmpeg on `PATH`, and Playwright with a Chromium:

```powershell
npm install --no-save playwright
npx playwright install chromium
```

Optional environment variables: `DEMO_CHROMIUM` (path to a Chromium or chrome-headless-shell),
`FFMPEG` (path to ffmpeg), `DEMO_PLAYWRIGHT_FROM` (a folder whose `node_modules` already holds Playwright).

## Changing the film

- **Words and timing:** edit the `TIMELINE` block at the top of the script in each page. `CAPTIONS` holds
  each caption with its start and end second, `PATH` the pointer's route, and `BEATS` when each layer
  (screen map, gate settings, tray menu, end card) is visible. `window.DEMO` sets size, frame rate and length.
- **The app pictures:** the gate settings and tray menu come straight from `docs/screenshots/`. After a UI
  change, regenerate them with the screenshot harness (see "Reproduce the generic screenshots" in the main
  README, and `--screenshot-tray <png> Dark en` for the tray menu), then re-render.
- **Colours and layout:** the CSS variables at the top of `scene.html`.

Open `scene.html` in a browser and call `renderAt(5)` in the console to check a single moment.
Keep the GIF under about 3 MB so GitHub shows it quickly.
