#!/bin/sh
# Run on an Apple Silicon build machine. Does not publish or replace an installed application.
set -eu
cd "$(dirname "$0")/.."
[ "$(uname -sm)" = 'Darwin arm64' ] || { echo 'Apple Silicon macOS required' >&2; exit 1; }
version=$(node -p 'require("./src-tauri/tauri.conf.json").version')
case "$version" in ''|*[!0-9.]*) exit 1;; esac
npm run tauri -- build --no-bundle
out=${1:-.artifacts/homebrew}; mkdir -p "$out"
stage=$(mktemp -d); trap 'rm -rf "$stage"' EXIT
# Build the release bundle in staging, not over a developer's running desktop app.
mkdir -p "$stage/target/release"
cp "${CARGO_TARGET_DIR:-src-tauri/target}/release/starpersona" "$stage/target/release/starpersona"
CARGO_TARGET_DIR="$stage/target" npm run tauri -- bundle --bundles app
/usr/bin/ditto "$stage/target/release/bundle/macos/StarPersona.app" "$stage/StarPersona.app"
# No Developer ID credentials are required for a local ad-hoc build; no notarization is implied.
/usr/bin/codesign --force --sign "${APPLE_SIGNING_IDENTITY:--}" "$stage/StarPersona.app"
/usr/bin/codesign --verify --strict "$stage/StarPersona.app"
name="StarPersona-$version-macos-arm64.zip"
/usr/bin/ditto -c -k --sequesterRsrc --keepParent "$stage/StarPersona.app" "$stage/$name"
sha=$(shasum -a 256 "$stage/$name" | cut -d ' ' -f1)
sed -e "s/@VERSION@/$version/g" -e "s/@SHA256@/$sha/g" packaging/starpersona.rb.in > "$stage/starpersona.rb"
cp "$stage/$name" "$stage/starpersona.rb" "$out/"
printf '%s  %s\n' "$sha" "$name" > "$out/SHA256SUMS"
