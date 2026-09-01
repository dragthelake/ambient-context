"""Build the macOS app icon from the pixel eye artwork.

The artwork (src/assets/app-icon.png) is a Windows 98 raised panel: a square
frame with a hard pixel bevel, bled to the canvas edge. Clipping that square
to the macOS squircle would slice the corners straight through the white
highlight band, so the bevel is redrawn to follow the squircle instead and
the eye is keyed out of the artwork and dropped inside it.

Geometry follows Apple's icon grid: a 1024 canvas with the art on an 824
square, so the icon sits at the same visual weight as its Dock neighbours.
The corner is a superellipse (n = 5), which tracks Apple's continuous corner
far more closely than a circular-arc rounded rect does.

Run: python3 tools/make-app-icon.py
Then: npx tauri icon src/assets/app-icon-macos.png -o src-tauri/icons
      (and delete the android/ and ios/ trees it writes; this app is macOS only)
"""

from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "src/assets/app-icon.png"
OUT = ROOT / "src/assets/app-icon-macos.png"

CANVAS = 1024
TILE = 824           # Apple's icon grid: 824 of 1024.
SUPER_N = 5.0        # Superellipse exponent approximating the macOS squircle.
SS = 4               # Supersampling factor for the tile edge and bands.
# Bevel band widths in canvas pixels, outermost first. Unequal, as the artwork
# is: its light outer band is about three times the white one, and evening them
# up turns the white into a halo that reads as an outline sticker.
BAND = [32, 20]

# The artwork's own bevel, sampled from it, outermost first; each entry is
# (light side, dark side). Not the pure black of --bevel-out: on a window that
# black is one hairline against a grey desktop, but here it is a 26px arc on
# transparency, and it reads as a drop shadow rather than an edge.
# The split is the anti-diagonal, as Windows miters it.
BANDS = [
    ((0xC0, 0xC0, 0xC0), (0x4D, 0x4D, 0x4D)),
    ((0xFF, 0xFF, 0xFF), (0x80, 0x80, 0x80)),
]
FACE = (0xB1, 0xB1, 0xB1)  # The artwork's own interior grey, so the eye's
                           # anti-aliased fringe lands on the colour it was
                           # drawn against.


def eye_sprite():
    """Key the eye out of the artwork, as RGBA at native resolution."""
    art = np.array(Image.open(SOURCE).convert("RGB")).astype(int)
    # The panel interior, inside the white highlight and above the shadow.
    inner = art[209:1806, 209:1806]

    # Background is the flat interior grey. Keying on colour alone punches
    # holes in the eye, which contains greys of its own, so the background is
    # taken as the region connected to the border instead.
    flat = np.abs(inner - np.array(FACE)).max(axis=2) <= 14
    labels, _ = ndimage.label(flat)
    border = set(labels[0].tolist() + labels[-1].tolist()
                 + labels[:, 0].tolist() + labels[:, -1].tolist()) - {0}
    background = np.isin(labels, list(border))

    # Keep only the eye itself. The artwork's interior is faintly vignetted, so
    # the key leaves specks that would otherwise stretch the bounding box and
    # shrink the eye to nothing.
    parts, count = ndimage.label(~background)
    if count > 1:
        sizes = ndimage.sum(~background, parts, range(1, count + 1))
        background = parts != (int(np.argmax(sizes)) + 1)

    ys, xs = np.nonzero(~background)
    top, bottom, left, right = ys.min(), ys.max() + 1, xs.min(), xs.max() + 1
    sprite = np.dstack([inner, np.where(background, 0, 255)]).astype(np.uint8)
    return Image.fromarray(sprite[top:bottom, left:right], "RGBA")


def squircle_radius(size, half):
    """Normalised superellipse radius: 1.0 exactly on the tile edge."""
    axis = (np.arange(size) - (size - 1) / 2.0) / half
    x = np.abs(axis)[None, :]
    y = np.abs(axis)[:, None]
    return (x ** SUPER_N + y ** SUPER_N) ** (1.0 / SUPER_N)


def tile():
    """The bevelled squircle, supersampled and reduced."""
    size = TILE * SS
    radius = squircle_radius(size, size / 2.0)

    axis = np.arange(size) - (size - 1) / 2.0
    # Light above the anti-diagonal (top and left), dark below it. The two
    # sides meet at the top-right and bottom-left corners, as Windows miters.
    lit = (axis[None, :] + axis[:, None]) < 0

    rgb = np.zeros((size, size, 3), dtype=np.uint8)
    rgb[:] = FACE
    outer = 1.0
    for width, (light, dark) in zip(BAND, BANDS):
        inner = outer - width * SS / (size / 2.0)
        ring = (radius <= outer) & (radius > inner)
        rgb[ring & lit] = light
        rgb[ring & ~lit] = dark
        outer = inner

    alpha = np.where(radius <= 1.0, 255, 0).astype(np.uint8)
    image = Image.fromarray(np.dstack([rgb, alpha]), "RGBA")
    return image.resize((TILE, TILE), Image.LANCZOS)


def main():
    canvas = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    art = tile()

    sprite = eye_sprite()
    # Fit the eye to 66% of the tile, which keeps it clear of the inner band
    # on every axis including the diagonals.
    target = int(TILE * 0.66)
    scale = target / max(sprite.size)
    sprite = sprite.resize(
        (round(sprite.width * scale), round(sprite.height * scale)),
        Image.LANCZOS,
    )
    art.alpha_composite(
        sprite,
        ((TILE - sprite.width) // 2, (TILE - sprite.height) // 2),
    )

    canvas.alpha_composite(art, ((CANVAS - TILE) // 2, (CANVAS - TILE) // 2))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(OUT)
    print(f"wrote {OUT.relative_to(ROOT)} {canvas.size}")


if __name__ == "__main__":
    main()
