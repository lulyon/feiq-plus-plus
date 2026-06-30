#!/bin/bash
# Generate all platform icons from icon.svg
# Prerequisites: brew install librsvg, python3 with Pillow
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SVG="$SCRIPT_DIR/icon.svg"
TMP="$(mktemp -d)"

echo "==> Generating icons from $SVG"

# --- PNG sizes for Tauri bundle ---
echo "  -> PNG (32x32)..."
rsvg-convert -w 32 -h 32 "$SVG" -o "$SCRIPT_DIR/32x32.png"

echo "  -> PNG (128x128)..."
rsvg-convert -w 128 -h 128 "$SVG" -o "$SCRIPT_DIR/128x128.png"

echo "  -> PNG (256x256 / 128x128@2x)..."
rsvg-convert -w 256 -h 256 "$SVG" -o "$SCRIPT_DIR/128x128@2x.png"

# --- macOS .icns via iconutil ---
echo "  -> ICNS (macOS)..."
ICONSET="$TMP/icon.iconset"
mkdir -p "$ICONSET"

set -- \
  "icon_16x16.png:16" "icon_16x16@2x.png:32" \
  "icon_32x32.png:32" "icon_32x32@2x.png:64" \
  "icon_128x128.png:128" "icon_128x128@2x.png:256" \
  "icon_256x256.png:256" "icon_256x256@2x.png:512" \
  "icon_512x512.png:512" "icon_512x512@2x.png:1024"

for pair; do
  name="${pair%%:*}"
  size="${pair##*:}"
  rsvg-convert -w "$size" -h "$size" "$SVG" -o "$ICONSET/$name"
done

iconutil -c icns "$ICONSET" -o "$SCRIPT_DIR/icon.icns"

# --- Windows .ico (multi-resolution) ---
echo "  -> ICO (Windows)..."
python3 - "$SVG" "$SCRIPT_DIR/icon.ico" "$TMP" << 'PYEOF'
import sys, struct, subprocess, os
svg, ico_out, tmp = sys.argv[1], sys.argv[2], sys.argv[3]
sizes = [16, 24, 32, 48, 64, 128, 256]
entries = []
offset = 6 + 16 * len(sizes)
for s in sizes:
    png = os.path.join(tmp, f"ico_{s}.png")
    subprocess.run(["rsvg-convert", "-w", str(s), "-h", str(s), svg, "-o", png], check=True)
    with open(png, "rb") as f:
        data = f.read()
    w = s if s < 256 else 0
    h = s if s < 256 else 0
    entries.append(struct.pack('<BBBBHHII', w, h, 0, 0, 1, 32, len(data), offset))
    entries.append(data)
    offset += len(data)
with open(ico_out, "wb") as f:
    f.write(struct.pack('<HHH', 0, 1, len(sizes)))
    for i in range(len(sizes)):
        f.write(entries[i])
    for i in range(len(sizes)):
        f.write(entries[len(sizes) + i])
print(f"    ICO created: {os.path.getsize(ico_out)} bytes with {len(sizes)} sizes")
PYEOF

rm -rf "$TMP"
echo ""
echo "==> Done! Generated:"
ls -lh "$SCRIPT_DIR"/{32x32.png,128x128.png,128x128@2x.png,icon.icns,icon.ico,icon.svg}
