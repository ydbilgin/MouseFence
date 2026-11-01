#!/usr/bin/env python3
"""Regenerate the README showcase blocks and the banner / social-preview art from docs/showcase/showcase.yml.

Usage (from the repository root):
    python3 docs/showcase/showcase.py            # rewrite README blocks, SVGs, and PNGs when a renderer is found
    python3 docs/showcase/showcase.py --no-png   # skip the PNG step
    python3 docs/showcase/showcase.py --check    # exit 1 if README blocks or SVGs are out of date (used by CI)

Python 3.8+ standard library only. showcase.yml is read with a small built-in reader that understands a
strict YAML subset: nested mappings, lists ("- "), plain or quoted scalars, '|' and '>' block scalars and
'#' comments. Anchors, flow collections ({...} / [...]) and multi-document files are not supported.
"""
import base64
import html
import os
import re
import shutil
import subprocess
import sys
import tempfile
import textwrap

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
CONFIG = os.path.join(HERE, "showcase.yml")


# --------------------------------------------------------------------------- YAML subset reader
class YamlError(ValueError):
    pass


def _strip_comment(line):
    out, quote = [], None
    for i, ch in enumerate(line):
        if quote:
            if ch == quote:
                quote = None
        elif ch in "\"'" and (i == 0 or line[i - 1] in " :-\t["):
            quote = ch
        elif ch == "#" and (i == 0 or line[i - 1] in " \t"):
            break
        out.append(ch)
    return "".join(out).rstrip()


def _scalar(text):
    t = text.strip()
    if not t:
        return None
    if t[0] == '"' and t[-1] == '"' and len(t) >= 2:
        return re.sub(r'\\(["\\nt])', lambda m: {"n": "\n", "t": "\t"}.get(m.group(1), m.group(1)), t[1:-1])
    if t[0] == "'" and t[-1] == "'" and len(t) >= 2:
        return t[1:-1].replace("''", "'")
    if t in ("true", "True"):
        return True
    if t in ("false", "False"):
        return False
    if t in ("null", "~"):
        return None
    if t[0] in "[{&*!|>":
        raise YamlError("unsupported YAML construct: %r" % t)
    return t


def load_yaml(text):
    raw = text.split("\n")
    lines = []  # (indent, content, raw_line_index)
    for n, line in enumerate(raw):
        if "\t" in line[: len(line) - len(line.lstrip())]:
            raise YamlError("line %d: tabs are not allowed for indentation" % (n + 1))
        stripped = _strip_comment(line)
        if stripped.strip():
            lines.append((len(stripped) - len(stripped.lstrip()), stripped.strip(), n))
    pos = [0]

    def block_scalar(style, parent_indent, n):
        body, indent = [], None
        i = n + 1
        while i < len(raw):
            line = raw[i]
            if line.strip():
                ind = len(line) - len(line.lstrip())
                if ind <= parent_indent:
                    break
                indent = ind if indent is None else indent
                body.append(line[indent:])
            else:
                body.append("")
            i += 1
        while pos[0] < len(lines) and lines[pos[0]][2] < i:
            pos[0] += 1
        while body and not body[-1]:
            body.pop()
        if style == "|":
            return "\n".join(body)
        paras, cur = [], []
        for b in body:
            if b:
                cur.append(b.strip())
            else:
                paras.append(" ".join(cur))
                cur = []
        paras.append(" ".join(cur))
        return "\n".join(p for p in paras)

    def parse_value(rest, indent, n):
        rest = rest.strip()
        if rest in ("|", ">", "|-", ">-"):
            return block_scalar(rest[0], indent, n)
        if rest:
            return _scalar(rest)
        if pos[0] < len(lines) and lines[pos[0]][0] > indent:
            return parse_node(lines[pos[0]][0])
        if pos[0] < len(lines) and lines[pos[0]][0] == indent and lines[pos[0]][1].startswith("- "):
            return parse_node(indent)
        return None

    def parse_map_entry(content, indent, n, target):
        m = re.match(r'^("[^"]*"|\'[^\']*\'|[^:]+?)\s*:(\s+(.*)|)$', content)
        if not m:
            raise YamlError("line %d: expected 'key: value'" % (n + 1))
        key = _scalar(m.group(1))
        if key in target:
            raise YamlError("line %d: duplicate key %r" % (n + 1, key))
        target[key] = parse_value(m.group(3) or "", indent, n)

    def parse_node(indent):
        first = lines[pos[0]][1]
        if first.startswith("- ") or first == "-":
            result = []
            while pos[0] < len(lines) and lines[pos[0]][0] == indent and (lines[pos[0]][1].startswith("- ") or lines[pos[0]][1] == "-"):
                ind, content, n = lines[pos[0]]
                pos[0] += 1
                item = content[1:].strip()
                if not item:
                    result.append(parse_value("", ind, n))
                elif re.match(r'^("[^"]*"|\'[^\']*\'|[^:"\']+?)\s*:(\s|$)', item):
                    obj = {}
                    child_indent = ind + 2
                    parse_map_entry(item, child_indent, n, obj)
                    while pos[0] < len(lines) and lines[pos[0]][0] == child_indent and not lines[pos[0]][1].startswith("- "):
                        _, c, nn = lines[pos[0]]
                        pos[0] += 1
                        parse_map_entry(c, child_indent, nn, obj)
                    result.append(obj)
                else:
                    result.append(_scalar(item))
            return result
        result = {}
        while pos[0] < len(lines) and lines[pos[0]][0] == indent:
            _, content, n = lines[pos[0]]
            if content.startswith("- "):
                raise YamlError("line %d: unexpected list item" % (n + 1))
            pos[0] += 1
            parse_map_entry(content, indent, n, result)
        if pos[0] < len(lines) and lines[pos[0]][0] > indent:
            raise YamlError("line %d: unexpected indentation" % (lines[pos[0]][2] + 1))
        return result

    if not lines:
        return {}
    data = parse_node(lines[0][0])
    if pos[0] != len(lines):
        raise YamlError("line %d: could not parse" % (lines[pos[0]][2] + 1))
    return data


# --------------------------------------------------------------------------- helpers
def esc(t):
    return html.escape("" if t is None else str(t), quote=True)


def cell(t):
    return ("" if t is None else str(t)).replace("|", "\\|").replace("\n", " ").strip()


def need(d, key, where):
    if not d.get(key):
        sys.exit("showcase.yml: '%s' is required in %s" % (key, where))
    return d[key]


def data_uri(path):
    full = os.path.join(ROOT, path)
    ext = os.path.splitext(path)[1].lower()
    mime = {".png": "image/png", ".svg": "image/svg+xml", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".gif": "image/gif"}.get(ext)
    if not mime:
        sys.exit("showcase.yml: icon must be .png, .svg, .jpg or .gif: %s" % path)
    with open(full, "rb") as f:
        return "data:%s;base64,%s" % (mime, base64.b64encode(f.read()).decode("ascii"))


def slug(text):
    return re.sub(r"[^a-z0-9]+", "-", str(text).lower()).strip("-")


# --------------------------------------------------------------------------- README blocks
def block_hero(c):
    h = c.get("hero") or {}
    out = []
    if h.get("banner"):
        out.append('<p align="center"><img src="%s" alt="%s: %s" width="100%%"></p>' % (esc(c["art"]["banner"]["out"]), esc(c["name"]), esc(c["tagline"])))
    else:
        if c.get("icon"):
            out.append('<p align="center"><img src="%s" alt="%s icon" width="%s"></p>' % (esc(c["icon"]), esc(c["name"]), esc(h.get("icon_width", 96))))
        out.append('<h1 align="center">%s</h1>' % esc(c["name"]))
    lead = "<strong>%s</strong>" % esc(c["tagline"])
    if c.get("description"):
        lead += "<br>\n  " + esc(c["description"])
    out.append('<p align="center">\n  %s\n</p>' % lead)
    badges = c.get("badges") or []
    if badges:
        parts = []
        for b in badges:
            img = '<img alt="%s" src="%s">' % (esc(need(b, "alt", "badges")), esc(need(b, "image", "badges")))
            parts.append('<a href="%s">%s</a>' % (esc(b["link"]), img) if b.get("link") else img)
        out.append('<p align="center">\n  %s\n</p>' % "\n  ".join(parts))
    if h.get("media"):
        out.append('<p align="center"><img src="%s" alt="%s" width="%s"></p>' % (esc(h["media"]), esc(h.get("alt", "")), esc(h.get("width", 800))))
        if h.get("caption"):
            out.append('<p align="center"><sub>%s</sub></p>' % esc(h["caption"]))
    nav = h.get("nav") or []
    if nav:
        out.append('<p align="center">\n  %s\n</p>' % " ·\n  ".join('<a href="%s"><b>%s</b></a>' % (esc(n["link"]), esc(n["label"])) for n in nav))
    return "\n".join(out)


def block_features(c):
    feats = c.get("features") or []
    if not feats:
        return ""
    head = c.get("features_header") or ["Feature", "What it does"]
    rows = ["| %s | %s |" % (cell(head[0]), cell(head[1])), "| --- | --- |"]
    for f in feats:
        text = cell(need(f, "text", "features"))
        if f.get("media"):
            text += " ([see it](%s))" % f["media"]
        rows.append("| **%s** | %s |" % (cell(need(f, "name", "features")), text))
    return "\n".join(rows)


def block_comparison(c):
    cmp_ = c.get("comparison")
    if not cmp_:
        return ""
    rows = ["| | %s | %s |" % (cell(need(cmp_, "builtin", "comparison")), cell(c["name"])), "| --- | --- | --- |"]
    for r in need(cmp_, "rows", "comparison"):
        rows.append("| **%s** | %s | %s |" % (cell(r.get("topic")), cell(r.get("builtin")), cell(r.get("tool"))))
    out = []
    if cmp_.get("intro"):
        out.append(cmp_["intro"].strip() + "\n")
    out.append("\n".join(rows))
    if cmp_.get("outro"):
        out.append("\n" + cmp_["outro"].strip())
    return "\n".join(out)


def block_gallery(c):
    media = c.get("gallery") or []
    if not media:
        return ""
    cols = int((c.get("gallery_columns") or 2))
    cells = []
    for m in media:
        cap = esc(m.get("caption", ""))
        cells.append('<td width="%d%%" valign="top"><img src="%s" alt="%s" width="100%%"><br><sub>%s</sub></td>' % (100 // cols, esc(need(m, "file", "gallery")), esc(m.get("alt", m.get("caption", ""))), cap))
    rows = ["<tr>%s</tr>" % "".join(cells[i:i + cols]) for i in range(0, len(cells), cols)]
    return "<table>\n%s\n</table>" % "\n".join(rows)


def block_cards(c):
    cards = c.get("cards") or []
    if not cards:
        return ""
    cols = int(c.get("card_columns") or 2)
    cells = []
    for card in cards:
        s = slug(card["name"])
        light, dark = card_paths(c, s)
        pic = ('<picture>\n<source media="(prefers-color-scheme: dark)" srcset="%s">\n<img alt="%s: %s" src="%s" width="100%%">\n</picture>'
               % (esc(dark), esc(card["name"]), esc(card["tagline"]), esc(light)))
        if card.get("link"):
            pic = '<a href="%s">\n%s\n</a>' % (esc(card["link"]), pic)
        cells.append('<td width="%d%%" valign="top">\n%s\n</td>' % (100 // cols, pic))
    rows = ["<tr>\n%s\n</tr>" % "\n".join(cells[i:i + cols]) for i in range(0, len(cells), cols)]
    return "<table>\n%s\n</table>" % "\n".join(rows)


BLOCKS = {"hero": block_hero, "features": block_features, "comparison": block_comparison, "gallery": block_gallery, "cards": block_cards}
MARKER = re.compile(r"(<!-- showcase:(\w+):start[^>]*-->\n)(.*?)(<!-- showcase:\2:end -->)", re.S)


def render_readme(text, c, path):
    seen = set()

    def repl(m):
        name = m.group(2)
        if name not in BLOCKS:
            sys.exit("%s: unknown showcase block '%s'" % (path, name))
        seen.add(name)
        body = BLOCKS[name](c)
        return m.group(1) + (body + "\n" if body else "") + m.group(4)

    return MARKER.sub(repl, text), seen


# --------------------------------------------------------------------------- art (banner, social preview, cards)
DEFAULT_SOCIAL = """<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="640" viewBox="0 0 1280 640">
<defs><linearGradient id="bg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{{bg1}}"/><stop offset="1" stop-color="{{bg2}}"/></linearGradient>
<radialGradient id="glow" cx="0.85" cy="0.1" r="0.7"><stop offset="0" stop-color="{{accent}}" stop-opacity="0.28"/><stop offset="1" stop-color="{{accent}}" stop-opacity="0"/></radialGradient></defs>
<rect width="1280" height="640" fill="url(#bg)"/><rect width="1280" height="640" fill="url(#glow)"/>
<rect width="1280" height="8" fill="{{accent}}"/>
<g font-family="Segoe UI, Inter, Helvetica, Arial, sans-serif">
<rect x="96" y="176" width="176" height="176" rx="40" fill="#ffffff" fill-opacity="0.06" stroke="#ffffff" stroke-opacity="0.10"/>
<image href="{{icon}}" x="112" y="192" width="144" height="144"/>
<text x="320" y="262" font-size="84" font-weight="700" fill="#F0F6FC">{{name}}</text>
<text x="320" y="330" font-size="40" fill="#C9D1D9">{{tagline_1}}</text>
<text x="320" y="384" font-size="40" fill="#C9D1D9">{{tagline_2}}</text>
<text x="320" y="438" font-size="40" fill="#C9D1D9">{{tagline_3}}</text>
<text x="96" y="560" font-size="26" fill="#8B949E">{{subtitle}}</text>
<text x="1184" y="560" font-size="26" fill="{{accent}}" text-anchor="end" font-weight="600">github.com/{{repo}}</text>
</g></svg>
"""

DEFAULT_BANNER = """<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="320" viewBox="0 0 1280 320">
<defs><linearGradient id="bg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{{bg1}}"/><stop offset="1" stop-color="{{bg2}}"/></linearGradient></defs>
<rect width="1280" height="320" rx="24" fill="url(#bg)"/>
<rect x="0" y="312" width="1280" height="8" fill="{{accent}}"/>
<g font-family="Segoe UI, Inter, Helvetica, Arial, sans-serif">
<image href="{{icon}}" x="80" y="88" width="144" height="144"/>
<text x="264" y="168" font-size="72" font-weight="700" fill="#F0F6FC">{{name}}</text>
<text x="264" y="224" font-size="32" fill="#C9D1D9">{{tagline}}</text>
</g></svg>
"""

CARD = """<svg xmlns="http://www.w3.org/2000/svg" width="420" height="132" viewBox="0 0 420 132">
<rect x="0.5" y="0.5" width="419" height="131" rx="10" fill="{{bg}}" stroke="{{border}}"/>
<rect x="0.5" y="0.5" width="5" height="131" rx="2" fill="{{accent}}"/>
<image href="{{icon}}" x="22" y="22" width="56" height="56"/>
<g font-family="-apple-system, Segoe UI, Helvetica, Arial, sans-serif">
<text x="96" y="40" font-size="18" font-weight="600" fill="{{title}}">{{name}}</text>
<text x="96" y="66" font-size="14" fill="{{text}}">{{tagline_1}}</text>
<text x="96" y="88" font-size="14" fill="{{text}}">{{tagline_2}}</text>
<circle cx="102" cy="111" r="6" fill="{{language_color}}"/>
<text x="114" y="116" font-size="13" fill="{{muted}}">{{language}}</text>
<text x="400" y="116" font-size="13" fill="{{muted}}" text-anchor="end">{{note}}</text>
</g></svg>
"""

CARD_THEMES = {
    "light": {"bg": "#ffffff", "border": "#d0d7de", "title": "#0969da", "text": "#1f2328", "muted": "#59636e"},
    "dark": {"bg": "#0d1117", "border": "#3d444d", "title": "#4493f8", "text": "#e6edf3", "muted": "#9198a1"},
}


def fill(template, values):
    def repl(m):
        key = m.group(1)
        if key not in values:
            sys.exit("template uses unknown placeholder {{%s}}" % key)
        v = values[key]
        return v if key in ("icon",) else esc(v)
    return re.sub(r"\{\{(\w+)\}\}", repl, template)


def art_values(c, a, wrap):
    lines = textwrap.wrap(c["tagline"], wrap) + ["", "", ""]
    return {
        "name": c["name"], "tagline": c["tagline"], "repo": c.get("repo", ""),
        "tagline_1": lines[0], "tagline_2": lines[1], "tagline_3": lines[2],
        "subtitle": a.get("subtitle", ""), "accent": c["accent"],
        "bg1": a.get("bg1", "#0D1117"), "bg2": a.get("bg2", "#161B22"),
        "icon": data_uri(c["icon"]) if c.get("icon") else "",
    }


def load_template(a, default):
    if a.get("template"):
        with open(os.path.join(ROOT, a["template"]), encoding="utf-8") as f:
            return f.read()
    return default


def card_paths(c, s):
    folder = c.get("card_dir", "assets/cards")
    return "%s/%s-light.svg" % (folder, s), "%s/%s-dark.svg" % (folder, s)


def art_outputs(c):
    """Return {relative_path: svg_text} plus a list of (svg, png, w, h) to rasterise."""
    files, pngs = {}, []
    art = c.get("art") or {}
    for kind, default, size in (("social", DEFAULT_SOCIAL, (1280, 640)), ("banner", DEFAULT_BANNER, (1280, 320))):
        a = art.get(kind)
        if not a:
            continue
        svg = fill(load_template(a, default), art_values(c, a, int(a.get("wrap", 34))))
        files[need(a, "out", "art." + kind)] = svg
        if a.get("png"):
            pngs.append((a["out"], a["png"], size[0], size[1]))
    for card in c.get("cards") or []:
        s = slug(card["name"])
        lines = textwrap.wrap(need(card, "tagline", "cards"), 44) + ["", ""]
        for theme, path in zip(("light", "dark"), card_paths(c, s)):
            v = dict(CARD_THEMES[theme])
            v.update({"name": card["name"], "tagline_1": lines[0], "tagline_2": lines[1],
                      "language": card.get("language", ""), "language_color": card.get("language_color", "#8b949e"),
                      "note": card.get("note", ""), "accent": card.get("accent", "#8b949e"),
                      "icon": data_uri(need(card, "icon", "cards"))})
            files[path] = fill(CARD, v)
    return files, pngs


def find_browser():
    names = ["chromium", "chromium-browser", "google-chrome", "chrome", "msedge", "microsoft-edge"]
    for n in names:
        p = shutil.which(n)
        if p:
            return p
    for p in (r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe", r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
              r"C:\Program Files\Google\Chrome\Application\chrome.exe", "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"):
        if os.path.exists(p):
            return p
    env = os.environ.get("SHOWCASE_BROWSER")
    return env if env and os.path.exists(env) else None


PLAYWRIGHT_JS = """
const { chromium } = require('playwright');
const fs = require('fs');
(async () => {
  const [svg, png, w, h] = process.argv.slice(1);
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: +w, height: +h } });
  await p.setContent('<html><body style="margin:0">' + fs.readFileSync(svg, 'utf8') + '</body></html>');
  await p.screenshot({ path: png, clip: { x: 0, y: 0, width: +w, height: +h } });
  await b.close();
})().catch(e => { console.error(e); process.exit(1); });
"""


def _try(cmd, **kw):
    try:
        subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=90, **kw)
        return True
    except (subprocess.SubprocessError, OSError):
        return False


def rasterise(svg_rel, png_rel, w, h):
    """SVG -> PNG with whatever is available: rsvg-convert, Node + Playwright, or headless Chrome/Edge."""
    svg, png = os.path.join(ROOT, svg_rel), os.path.join(ROOT, png_rel)
    if shutil.which("rsvg-convert") and _try(["rsvg-convert", "-w", str(w), "-h", str(h), "-o", png, svg]):
        return "rsvg-convert"
    if shutil.which("node") and _try(["node", "-e", PLAYWRIGHT_JS, svg, png, str(w), str(h)]):
        return "node + playwright"
    browser = find_browser()
    if browser:
        with tempfile.TemporaryDirectory() as tmp:
            page = os.path.join(tmp, "page.html")
            with open(svg, encoding="utf-8") as f:
                body = f.read()
            with open(page, "w", encoding="utf-8") as f:
                f.write('<!doctype html><html><body style="margin:0;overflow:hidden">%s</body></html>' % body)
            ok = _try([browser, "--headless", "--disable-gpu", "--hide-scrollbars", "--no-first-run",
                       "--user-data-dir=" + os.path.join(tmp, "profile"), "--window-size=%d,%d" % (w, h),
                       "--screenshot=" + os.path.abspath(png), "file:///" + page.replace("\\", "/").lstrip("/")])
        if ok and os.path.exists(png):
            return os.path.basename(browser)
    return None


# --------------------------------------------------------------------------- main
def main(argv):
    check = "--check" in argv
    with open(CONFIG, encoding="utf-8") as f:
        c = load_yaml(f.read())
    for key in ("name", "tagline", "accent"):
        need(c, key, "showcase.yml")
    stale = []
    for readme in c.get("readmes") or ["README.md"]:
        path = os.path.join(ROOT, readme)
        with open(path, encoding="utf-8") as f:
            old = f.read()
        new, _ = render_readme(old, c, readme)
        if new != old:
            stale.append(readme)
            if not check:
                with open(path, "w", encoding="utf-8", newline="\n") as f:
                    f.write(new)
    files, pngs = art_outputs(c)
    for rel, text in files.items():
        path = os.path.join(ROOT, rel)
        old = open(path, encoding="utf-8").read() if os.path.exists(path) else None
        if old != text:
            stale.append(rel)
            if not check:
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path, "w", encoding="utf-8", newline="\n") as f:
                    f.write(text)
    if check:
        if stale:
            print("Out of date with docs/showcase/showcase.yml: " + ", ".join(stale))
            print("Run: python3 docs/showcase/showcase.py   and commit the result.")
            return 1
        print("Showcase is in sync with docs/showcase/showcase.yml.")
        return 0
    for rel in stale:
        print("updated " + rel)
    if "--no-png" not in argv:
        for svg_rel, png_rel, w, h in pngs:
            tool = rasterise(svg_rel, png_rel, w, h)
            print(("rendered %s with %s" % (png_rel, tool)) if tool else
                  "skipped %s: no renderer found (install rsvg-convert, or Chrome/Edge, or set SHOWCASE_BROWSER)" % png_rel)
    if not stale:
        print("Nothing to update.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
