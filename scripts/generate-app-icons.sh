#!/usr/bin/env bash
# Generate Freedesktop hicolor icons from assets/app-icon/source.png.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE="${ROOT}/assets/app-icon/source.png"
OUT_BASE="${ROOT}/assets/icons/hicolor"
ICON_NAME="dev.gitforge.GitForge"
SIZES=(16 32 48 64 128 256 512)

if [[ ! -f "${SOURCE}" ]]; then
  echo "error: missing source icon at ${SOURCE}" >&2
  exit 1
fi

if command -v magick >/dev/null 2>&1; then
  CONVERT=(magick)
elif command -v convert >/dev/null 2>&1; then
  CONVERT=(convert)
else
  CONVERT=()
fi

generate_imagemagick() {
  local size="$1"
  local out_dir="${OUT_BASE}/${size}x${size}/apps"
  local out_file="${out_dir}/${ICON_NAME}.png"
  mkdir -p "${out_dir}"
  # Fuzzy-trim near-transparent halo, then scale to fill the square canvas.
  "${CONVERT[@]}" "${SOURCE}" \
    -fuzz 1% \
    -trim +repage \
    -resize "${size}x${size}" \
    -background none \
    -gravity center \
    -extent "${size}x${size}" \
    "${out_file}"
  echo "  ${out_file}"
}

generate_pillow() {
  python3 - "${SOURCE}" "${OUT_BASE}" "${ICON_NAME}" "${SIZES[@]}" <<'PY'
import sys
from pathlib import Path

from PIL import Image

source_path = Path(sys.argv[1])
out_base = Path(sys.argv[2])
icon_name = sys.argv[3]
sizes = [int(s) for s in sys.argv[4:]]

img = Image.open(source_path).convert("RGBA")
# Match ImageMagick -fuzz 1%: ignore near-transparent fringe pixels.
alpha = img.getchannel("A")
mask = alpha.point(lambda a: 255 if a > 3 else 0)
if bbox := mask.getbbox():
    img = img.crop(bbox)

for size in sizes:
    w, h = img.size
    scale = size / max(w, h)
    new_w = max(1, round(w * scale))
    new_h = max(1, round(h * scale))
    scaled = img.resize((new_w, new_h), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    offset = ((size - new_w) // 2, (size - new_h) // 2)
    canvas.paste(scaled, offset, scaled)
    out_dir = out_base / f"{size}x{size}" / "apps"
    out_dir.mkdir(parents=True, exist_ok=True)
    out_file = out_dir / f"{icon_name}.png"
    canvas.save(out_file, optimize=True)
    print(f"  {out_file}")
PY
}

echo "Generating hicolor icons from ${SOURCE}..."
if ((${#CONVERT[@]} > 0)); then
  for size in "${SIZES[@]}"; do
    generate_imagemagick "${size}"
  done
else
  if ! python3 -c "import PIL" 2>/dev/null; then
    echo "error: need ImageMagick (magick/convert) or Python Pillow" >&2
    exit 1
  fi
  generate_pillow
fi

echo "Done."
