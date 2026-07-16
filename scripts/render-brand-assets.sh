#!/usr/bin/env bash
set -euo pipefail

command -v magick >/dev/null || {
  printf 'ImageMagick 7 is required to render Deckmark PNGs\n' >&2
  exit 1
}

render() {
  local source=$1 size=$2 output=$3
  magick -background none "$source" -resize "$size" -strip \
    -define png:exclude-chunk=date,time "$output"
}

render brand/deckmark-horizontal.svg 720x160 brand/deckmark-horizontal-720.png
render brand/deckmark-compact.svg 256x256 brand/deckmark-compact-256.png
render brand/deckmark-compact.svg 64x64 brand/deckmark-compact-64.png
render brand/deckmark-favicon.svg 16x16 brand/deckmark-favicon-16.png
render brand/deckmark-monochrome-dark.svg 256x256 brand/deckmark-monochrome-dark-256.png
render brand/deckmark-monochrome-light.svg 256x256 brand/deckmark-monochrome-light-256.png

if rg -n '<text|href=|url\(|linearGradient|radialGradient|font-family' brand/*.svg; then
  printf 'Deckmark SVGs must contain only local, font-free, flat geometry\n' >&2
  exit 1
fi

(
  cd brand
  sha256sum -- deckmark-*.svg deckmark-*.png | LC_ALL=C sort -k2
) > brand/SHA256SUMS
