# The README demo film

`docs/demo.gif` is drawn, not recorded. `scene.html` is a small web page that paints any moment of the
film when asked (`window.renderAt(seconds)`), and `render.mjs` steps through it frame by frame in a headless
Chromium and lets ffmpeg turn the frames into an optimised GIF. Nobody's screen is captured.

## Re-render in one command

From the repository root:

```powershell
node docs/demo/render.mjs --sheet
```

This rewrites `docs/demo.gif` and writes `docs/demo/contact-sheet.png` (one frame per second, for a quick
look; it is git-ignored). Add `--mp4` for `docs/demo.mp4` as well.

Needs Node.js 18 or newer, ffmpeg on `PATH`, and Playwright with a Chromium:

```powershell
npm install --no-save playwright
npx playwright install chromium
```

Optional environment variables: `DEMO_CHROMIUM` (path to a Chromium or chrome-headless-shell),
`FFMPEG` (path to ffmpeg), `DEMO_PLAYWRIGHT_FROM` (a folder whose `node_modules` already holds Playwright).

## Changing the film

- **Words and timing:** edit the `TIMELINE` block at the top of the script in `scene.html`. `CAPTIONS` holds
  each caption with its start and end second, `PATH` the pointer's route, and `BEATS` when each layer
  (screen map, gate settings, tray menu, end card) is visible. `window.DEMO` sets size, frame rate and length.
- **The app pictures:** the gate settings and tray menu come straight from `docs/screenshots/`. After a UI
  change, regenerate them with the screenshot harness (see "Reproduce the generic screenshots" in the main
  README, and `--screenshot-tray <png> Dark en` for the tray menu), then re-render.
- **Colours and layout:** the CSS variables at the top of `scene.html`.

Open `scene.html` in a browser and call `renderAt(5)` in the console to check a single moment.
Keep the GIF under about 3 MB so GitHub shows it quickly.
