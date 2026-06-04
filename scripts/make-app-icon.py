#!/usr/bin/env python3
"""Turn one source image into a macOS app icon (.icns) — and, optionally, Linux
and web icons.

Why this exists: unlike iOS, macOS does **not** auto-mask app icons into the
rounded "squircle" tile — the rounding, inset padding and drop shadow have to be
baked into the artwork. This takes any square-ish source (a render, a photo, a
logo) and bakes them in, then emits a multi-resolution `.icns`.

Usage:
    python3 scripts/make-app-icon.py [SOURCE] [options]

    SOURCE              source image. Default: assets/icon-source.png
                        A .svg source is rasterized first (cairosvg, else qlmanage).

Options:
    --name NAME         base name for Linux / .desktop output (default: deck)
    --out DIR           assets dir for icon.png + icon.icns (default: assets)
    --no-mask           source is already a finished tile — skip squircle/pad/shadow
    --no-shadow         squircle + padding, but no drop shadow
    --linux             also emit a freedesktop hicolor tree + .desktop under dist/linux
    --web               also emit a full-bleed rounded PNG: dist/icon-rounded.png

Requires: Pillow (`pip install pillow`). The .icns step uses macOS `iconutil`.
"""
import argparse
import math
import os
import subprocess
import sys

try:
    from PIL import Image, ImageDraw, ImageFilter
except ImportError:
    sys.exit("error: Pillow is required — run `pip install pillow`")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SS = 4  # supersample factor for crisp, anti-aliased mask edges


def squircle_mask(size, n=5.0):
    """Apple-style continuous-corner squircle as an 'L' alpha mask, size x size.

    A superellipse |x|^n + |y|^n = 1 (n~5 matches Apple's curve) sampled as a
    high-res polygon, then downscaled for clean anti-aliasing — pure Pillow, no
    numpy."""
    hi = size * SS
    r = (hi - 1) / 2.0
    pts, steps = [], 720
    for i in range(steps):
        t = 2.0 * math.pi * i / steps
        ct, st = math.cos(t), math.sin(t)
        x = r + r * math.copysign(abs(ct) ** (2.0 / n), ct)
        y = r + r * math.copysign(abs(st) ** (2.0 / n), st)
        pts.append((x, y))
    m = Image.new("L", (hi, hi), 0)
    ImageDraw.Draw(m).polygon(pts, fill=255)
    return m.resize((size, size), Image.LANCZOS)


def rasterize_svg(src, tmp_png):
    """Best-effort SVG -> 1024 PNG using cairosvg, else macOS qlmanage."""
    if subprocess.run(["which", "cairosvg"], capture_output=True).returncode == 0:
        subprocess.run(["cairosvg", src, "-o", tmp_png, "-W", "1024", "-H", "1024"], check=True)
    else:
        d = os.path.dirname(tmp_png) or "."
        subprocess.run(["qlmanage", "-t", "-s", "1024", "-o", d, src],
                       check=True, capture_output=True)
        produced = os.path.join(d, os.path.basename(src) + ".png")
        os.replace(produced, tmp_png)
    return tmp_png


def load_source(src):
    if src.lower().endswith(".svg"):
        tmp = os.path.join(ROOT, "dist", "_icon-source.png")
        os.makedirs(os.path.dirname(tmp), exist_ok=True)
        src = rasterize_svg(src, tmp)
    im = Image.open(src).convert("RGBA")
    return im.resize((1024, 1024), Image.LANCZOS) if im.size != (1024, 1024) else im


def masked_tile(art, body, shadow):
    """1024 canvas: a `body`-px squircle tile centered, with an optional shadow."""
    canvas = 1024
    margin = (canvas - body) // 2
    tile = art.resize((body, body), Image.LANCZOS)
    mask = squircle_mask(body)
    tile.putalpha(mask)

    out = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    if shadow:
        sh_alpha = Image.new("L", (canvas, canvas), 0)
        sh_alpha.paste(mask, (margin, margin))
        sh = Image.new("RGBA", (canvas, canvas), (20, 18, 30, 255))
        sh.putalpha(sh_alpha.point(lambda p: int(p * 0.32)))
        sh = sh.filter(ImageFilter.GaussianBlur(18))
        out.alpha_composite(sh, (0, 12))
    out.alpha_composite(tile, (margin, margin))
    return out


def write_icns(master, out_dir):
    iconset = os.path.join(out_dir, "icon.iconset")
    os.makedirs(iconset, exist_ok=True)
    for sz, name in [(16, "icon_16x16.png"), (32, "icon_16x16@2x.png"),
                     (32, "icon_32x32.png"), (64, "icon_32x32@2x.png"),
                     (128, "icon_128x128.png"), (256, "icon_128x128@2x.png"),
                     (256, "icon_256x256.png"), (512, "icon_256x256@2x.png"),
                     (512, "icon_512x512.png"), (1024, "icon_512x512@2x.png")]:
        master.resize((sz, sz), Image.LANCZOS).save(os.path.join(iconset, name))
    icns = os.path.join(out_dir, "icon.icns")
    subprocess.run(["iconutil", "-c", "icns", iconset, "-o", icns], check=True)
    subprocess.run(["rm", "-rf", iconset])
    return icns


def write_linux(fullbleed, name):
    root = os.path.join(ROOT, "dist", "linux")
    made = []
    for s in [16, 22, 24, 32, 48, 64, 128, 256, 512]:
        d = os.path.join(root, "hicolor", f"{s}x{s}", "apps")
        os.makedirs(d, exist_ok=True)
        p = os.path.join(d, f"{name}.png")
        fullbleed.resize((s, s), Image.LANCZOS).save(p)
        made.append(p)
    desktop = os.path.join(root, f"{name}.desktop")
    with open(desktop, "w") as f:
        f.write("[Desktop Entry]\nType=Application\nName=Deck\n"
                "Comment=Native cross-platform desktop app starter built on GPUI\n"
                f"Exec={name}\nIcon={name}\nTerminal=false\n"
                "Categories=Development;Utility;\n")
    return root


def main():
    ap = argparse.ArgumentParser(description="Generate macOS/Linux/web app icons from one image.")
    ap.add_argument("source", nargs="?", default=os.path.join(ROOT, "assets", "icon-source.png"))
    ap.add_argument("--name", default="deck")
    ap.add_argument("--out", default=os.path.join(ROOT, "assets"))
    ap.add_argument("--no-mask", action="store_true")
    ap.add_argument("--no-shadow", action="store_true")
    ap.add_argument("--linux", action="store_true")
    ap.add_argument("--web", action="store_true")
    a = ap.parse_args()

    if not os.path.exists(a.source):
        sys.exit(f"error: source not found: {a.source}\n"
                 "Drop a 1024x1024 image at assets/icon-source.png (or pass a path).")
    os.makedirs(a.out, exist_ok=True)

    art = load_source(a.source)
    # macOS master: 824/1024 squircle tile + padding + shadow (Apple grid),
    # or the raw art if --no-mask (caller supplied a finished tile).
    master = art if a.no_mask else masked_tile(art, body=824, shadow=not a.no_shadow)
    master_png = os.path.join(a.out, "icon.png")
    master.save(master_png)
    icns = write_icns(master, a.out)
    print(f"icon.png  -> {master_png}")
    print(f"icon.icns -> {icns}")

    if a.web or a.linux:
        fullbleed = art if a.no_mask else art.copy()
        if not a.no_mask:
            fullbleed.putalpha(squircle_mask(1024))
        if a.web:
            web_dir = os.path.join(ROOT, "dist")
            os.makedirs(web_dir, exist_ok=True)
            web = os.path.join(web_dir, "icon-rounded.png")
            fullbleed.save(web)
            print(f"web       -> {web}")
        if a.linux:
            print(f"linux     -> {write_linux(fullbleed, a.name)}")


if __name__ == "__main__":
    main()
