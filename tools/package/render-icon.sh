#!/bin/sh
# Renders assets/icon/nomux.svg to an icon file of macOS (.icns), with the
# ten images of an iconset. Runs on macOS: needs rsvg-convert (Homebrew
# package librsvg) and iconutil (part of macOS).
#
# Usage: tools/package/render-icon.sh <output .icns>
set -eu

output=$1
source=assets/icon/nomux.svg
iconset=$(mktemp -d)/nomux.iconset
mkdir "$iconset"

for side in 16 32 128 256 512; do
  rsvg-convert --width "$side" --height "$side" --output "$iconset/icon_${side}x${side}.png" "$source"
  double=$((side * 2))
  rsvg-convert --width "$double" --height "$double" --output "$iconset/icon_${side}x${side}@2x.png" "$source"
done

iconutil --convert icns --output "$output" "$iconset"
rm -rf "$(dirname "$iconset")"
