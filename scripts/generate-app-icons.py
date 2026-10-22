#!/usr/bin/env python3
"""Convert the existing Cikung orange head to native icon formats (Pillow required)."""
from pathlib import Path
from PIL import Image

root = Path(__file__).resolve().parent.parent
folder = root / "assets/icons"
folder.mkdir(exist_ok=True)
with Image.open(root / "assets/cikung-face.png") as source:
    image = source.convert("RGBA")
    assert image.width == image.height and image.getpixel((0, 0))[3] == 0
    image.save(folder / "cikung.ico", format="ICO", sizes=[(n, n) for n in (16, 24, 32, 48, 64, 128, 256)])
    image.resize((1024, 1024), Image.Resampling.LANCZOS).save(folder / "Cikung.icns", format="ICNS")
print("Native icons generated from assets/cikung-face.png")
