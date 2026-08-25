#!/usr/bin/env bash
# Build the Rust scanner into this plugin folder.
# Safe to run after `omarchy plugin add` (clone is already the plugin dir)
# or from a development checkout (copies into ~/.config/omarchy/plugins/).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
ID="codechap.grokbar"
DEST="${HOME}/.config/omarchy/plugins/${ID}"

cd "$ROOT"
command -v cargo >/dev/null || {
  echo "install.sh: cargo not found. Install Rust (rustup) and retry." >&2
  exit 1
}

cargo build --release

install_bin() {
  local src="$1" dest="$2"
  if install -m 755 "$src" "$dest" 2>/dev/null; then
    return 0
  fi
  # Scanner may already be mapped by omarchy-shell; replace via rename.
  local tmp="${dest}.new.$$"
  cp "$src" "$tmp"
  chmod 755 "$tmp"
  mv -f "$tmp" "$dest"
}

install_bin "$ROOT/target/release/grokbar" "$ROOT/grokbar"

if [[ "$ROOT" != "$DEST" ]]; then
  mkdir -p "$DEST/assets"
  install_bin "$ROOT/grokbar" "$DEST/grokbar"
  cp -a "$ROOT"/*.qml "$ROOT/manifest.json" "$DEST/"
  cp -a "$ROOT/assets/." "$DEST/assets/"
  [[ -f "$ROOT/LICENSE" ]] && cp -a "$ROOT/LICENSE" "$DEST/LICENSE"
  echo "Installed ${ID} -> ${DEST}"
else
  echo "Built grokbar in ${ROOT}"
fi

if command -v omarchy >/dev/null; then
  omarchy plugin validate "$ROOT" || true
fi
