# Updating the README showcase

`docs/showcase/showcase.yml` is the single source of truth for the tagline, accent colour, badges, hero media,
feature table, built-in comparison (where there is one), gallery and the banner / social-preview art.

1. Edit `docs/showcase/showcase.yml` (add or remove a feature, screenshot, badge or comparison row).
2. Run `python3 docs/showcase/showcase.py` from the repository root (Python 3.8+, standard library only).
   It rewrites the README parts between `<!-- showcase:NAME:start -->` and `<!-- showcase:NAME:end -->` and the SVG art,
   and renders the PNGs if it finds `rsvg-convert`, Node with Playwright, or Chrome/Edge (`SHOWCASE_BROWSER=<path>` to choose one).
3. Commit the YAML together with the regenerated files. CI runs `python3 docs/showcase/showcase.py --check`
   and fails if the README blocks or SVGs no longer match the YAML.

Text outside the markers is written by hand and never touched by the script. Do not edit inside the markers;
the next run overwrites it. Blocks: `hero`, `features`, `comparison`, `gallery` (and `cards` for the profile).
Art can use a custom SVG template (`art.social.template`) with `{{name}}`, `{{tagline_1..3}}`, `{{accent}}`, `{{icon}}`,
`{{subtitle}}`, `{{repo}}`, `{{bg1}}`, `{{bg2}}` placeholders. The YAML reader supports a strict subset: mappings,
`- ` lists, plain or quoted values, `|` / `>` blocks and `#` comments (no `{}` / `[]` inline collections).
