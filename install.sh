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
install -m 755 "$ROOT/target/release/grokbar" "$ROOT/grokbar"

if [[ "$ROOT" != "$DEST" ]]; then
  mkdir -p "$DEST/assets"
  install -m 755 "$ROOT/grokbar" "$DEST/grokbar"
  cp -a "$ROOT/BarWidget.qml" "$ROOT/Panel.qml" "$ROOT/manifest.json" "$DEST/"
  cp -a "$ROOT/assets/." "$DEST/assets/"
  [[ -f "$ROOT/LICENSE" ]] && cp -a "$ROOT/LICENSE" "$DEST/LICENSE"
  echo "Installed ${ID} -> ${DEST}"
else
  echo "Built grokbar in ${ROOT}"
fi

if command -v omarchy >/dev/null; then
  omarchy plugin validate "$ROOT" || true
fi
