#!/bin/sh
set -eu

cd "$(dirname "$0")"

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "wasm-pack not found; install with: cargo install wasm-pack" >&2
  exit 1
fi

wasm-pack build ../../crates/urlwasher-wasm --target web --release \
  --out-dir ../../extensions/browser/wasm \
  --out-name urldebloater_urlwasher_wasm --no-typescript

version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' ../../Cargo.toml | head -n 1)
test -n "$version"

for target in chrome firefox; do
  rm -rf "dist/$target"
  mkdir -p "dist/$target"
  cp background.js settings.js options.html options.js "dist/$target/"
  if [ "$target" = chrome ]; then
    cp wait.html wait.js "dist/$target/"
  fi
  sed "s/\"version\": \"[^\"]*\"/\"version\": \"$version\"/" \
    "manifest.$target.json" > "dist/$target/manifest.json"
  cp -R wasm "dist/$target/wasm"
  cp -R _locales "dist/$target/_locales"
  cp ../../assets/icon/tray.png "dist/$target/icon.png"
  rm -f "dist/urldebloater-$target.zip"
  (cd "dist/$target" && zip -qr "../urldebloater-$target.zip" .)
done
