#!/usr/bin/env bash
#
# Build the universal macOS DMG — and refuse to produce a broken one.
#
# `tauri build --target universal-apple-darwin` cannot be trusted on its own
# here. This crate ships two binaries (the GUI app and the standalone CLI), and
# Tauri's universal build only runs `lipo` on the one named after the *package*
# while its bundler expects every [[bin]] to already exist as a universal
# binary. Until v0.8.3 the CLI held the package name, so the DMG was bundled
# with the CLI as the app executable — it installed and launched into nothing.
#
# So: build every bin for both arches, lipo every bin, bundle, then verify the
# result really is the GUI and really is universal before anyone ships it.

set -euo pipefail

cd "$(dirname "$0")/.."

ARCHS=(aarch64-apple-darwin x86_64-apple-darwin)
UNIVERSAL=src-tauri/target/universal-apple-darwin/release
APP="$UNIVERSAL/bundle/macos/Alpheus.app"

# Every [[bin]] in the crate, read from Cargo.toml rather than hardcoded, so
# adding a binary cannot silently reintroduce the bug.
BINS=($(awk '/^\[\[bin\]\]/{f=1;next} f && /^[[:space:]]*name[[:space:]]*=/{ sub(/^[^"]*"/,""); sub(/".*$/,""); print; f=0 }' src-tauri/Cargo.toml))
if [ ${#BINS[@]} -eq 0 ]; then
  echo "error: no [[bin]] targets found in src-tauri/Cargo.toml" >&2
  exit 1
fi
echo "==> binaries: ${BINS[*]}"

echo "==> frontend"
pnpm install --frozen-lockfile
pnpm build

for target in "${ARCHS[@]}"; do
  echo "==> cargo build --release --bins --target $target"
  cargo build --release --bins --target "$target" --manifest-path src-tauri/Cargo.toml
done

echo "==> lipo"
mkdir -p "$UNIVERSAL"
for bin in "${BINS[@]}"; do
  lipo -create -output "$UNIVERSAL/$bin" \
    "src-tauri/target/${ARCHS[0]}/release/$bin" \
    "src-tauri/target/${ARCHS[1]}/release/$bin"
done

echo "==> bundle"
pnpm tauri build --target universal-apple-darwin

# ---------------------------------------------------------------- verify
echo "==> verify"

main=$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")
exe="$APP/Contents/MacOS/$main"
[ -f "$exe" ] || { echo "FAIL: CFBundleExecutable '$main' is not in the bundle" >&2; exit 1; }

# The app executable must be the GUI. The CLI's usage banner in there means the
# bundler picked the wrong binary again.
#
# Counted rather than `grep -q`: under `set -o pipefail` an early-exiting grep
# SIGPIPEs `strings` and the whole pipeline reports failure, which reads as a
# broken bundle when the bundle is fine.
cli_markers=$(strings -a "$exe" | grep -cE "ncdu-style|Generate shell auto-completions" || true)
gui_markers=$(strings -a "$exe" | grep -cE "tauri://localhost|__TAURI" || true)

if [ "$cli_markers" -gt 0 ]; then
  echo "FAIL: '$main' is the CLI, not the GUI app" >&2
  exit 1
fi
if [ "$gui_markers" -eq 0 ]; then
  echo "FAIL: '$main' does not look like a Tauri GUI binary" >&2
  exit 1
fi

# Both slices, in the app and in every binary beside it.
for bin in "${BINS[@]}"; do
  [ -f "$APP/Contents/MacOS/$bin" ] || continue
  info=$(lipo -info "$APP/Contents/MacOS/$bin")
  case "$info" in
    *x86_64*arm64*|*arm64*x86_64*) ;;
    *) echo "FAIL: $bin is not universal — $info" >&2; exit 1 ;;
  esac
done

codesign --verify --deep --strict "$APP" || { echo "FAIL: signature invalid" >&2; exit 1; }

version=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$APP/Contents/Info.plist")
dmg=$(ls "$UNIVERSAL"/bundle/dmg/*.dmg | head -1)

echo
echo "OK  Alpheus $version"
echo "    app: $APP  (executable: $main, universal)"
echo "    dmg: $dmg"
