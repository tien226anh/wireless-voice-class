#!/usr/bin/env python3
"""Render assets/logo.svg into the app PNG and a multi-resolution Windows ICO.

Ubuntu/Debian: apt install python3-gi python3-gi-cairo gir1.2-rsvg-2.0 python3-pil
Run with /usr/bin/python3 scripts/generate-icons.py.
The generated files are committed; ordinary builds do not need these tools.
"""

from io import BytesIO
from pathlib import Path

import cairo
import gi
from PIL import Image

gi.require_version("Rsvg", "2.0")
from gi.repository import Rsvg

ROOT = Path(__file__).resolve().parents[1]
SIZES = (16, 24, 32, 48, 64, 128, 256)


def render(size):
    surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, size, size)
    viewport = Rsvg.Rectangle()
    viewport.x = viewport.y = 0
    viewport.width = viewport.height = size
    logo = Rsvg.Handle.new_from_file(str(ROOT / "assets/logo.svg"))
    logo.render_document(cairo.Context(surface), viewport)
    output = BytesIO()
    surface.write_to_png(output)
    output.seek(0)
    return Image.open(output).convert("RGBA")


def main():
    icons = ROOT / "assets/icons"
    icons.mkdir(exist_ok=True)
    render(512).save(icons / "wireless-pa.png")
    # Render each size from the vector source to keep small icons crisp.
    frames = [render(size) for size in SIZES]
    frames[-1].save(icons / "wireless-pa.ico", sizes=[(s, s) for s in SIZES],
                    append_images=frames[:-1])
    with Image.open(icons / "wireless-pa.ico") as icon:
        assert icon.ico.sizes() == {(s, s) for s in SIZES}
    print("Generated 512px PNG and Windows ICO (16–256px) from assets/logo.svg")


if __name__ == "__main__":
    main()
