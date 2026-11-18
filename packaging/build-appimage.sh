#!/usr/bin/env bash
# Собирает ps5emu-x86_64.AppImage из target/release/ps5emu.
# Запуск: cargo build --release && bash packaging/build-appimage.sh
set -euo pipefail
cd "$(dirname "$0")/.."

APPDIR=build/AppDir
rm -rf build
mkdir -p "$APPDIR/usr/bin"

cp target/release/ps5emu "$APPDIR/usr/bin/ps5emu"
cp packaging/ps5emu.desktop "$APPDIR/ps5emu.desktop"
cp packaging/AppRun "$APPDIR/AppRun"
chmod +x "$APPDIR/AppRun"
cp assets/ps5emu.png "$APPDIR/ps5emu.png"
cp assets/ps5emu.png "$APPDIR/.DirIcon"

TOOL=build/appimagetool
curl -fsSL -o "$TOOL" \
  https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x "$TOOL"

ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" "$APPDIR" ps5emu-x86_64.AppImage
echo "Готово: ps5emu-x86_64.AppImage"
