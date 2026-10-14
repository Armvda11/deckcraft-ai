#!/bin/sh
# Libertinus Math v7.051, OFL-1.1. Fonts remain in the external craft-fonts checkout.
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
font_dir=${CRAFT_FONTS_DIR:-"$root/../craft-fonts"}/fonts/libertinus
archive=$(mktemp)
trap 'rm -f "$archive"' EXIT
curl -fsSL https://github.com/alerque/libertinus/releases/download/v7.051/Libertinus-7.051.zip -o "$archive"
mkdir -p "$font_dir"
unzip -p "$archive" '*/static/OTF/LibertinusMath-Regular.otf' > "$font_dir/LibertinusMath-Regular.otf"
unzip -p "$archive" '*/OFL.txt' > "$font_dir/OFL.txt"
echo 'ded714b0d3808af527438ed7a85f16933133f66a12104c4a83db4cc248888011  LibertinusMath-Regular.otf' > "$font_dir/SHA256SUMS"
(cd "$font_dir" && shasum -a 256 -c SHA256SUMS)
