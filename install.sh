#!/usr/bin/env bash
# Builds cyberwatch and installs it as a LOCAL-ONLY Omarchy bar-widget
# plugin — never through `omarchy plugin add` (that's the git/marketplace-
# style installer) and never submitted to the plugin marketplace. Same
# mechanism this box already uses for a couple of other personal plugins:
# a hand-placed `custom.<name>` folder under ~/.config/omarchy/plugins/,
# containing just manifest.json + BarWidget.qml (symlinked back to this
# repo's copies, so a future `git pull` here updates the live plugin
# without rerunning this script for QML/manifest-only changes).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

echo "Building cyberwatch (release)..."
cargo build --release

if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
  BIN_SRC="${CARGO_TARGET_DIR}/release/cyberwatch"
elif [[ -f "$HOME/.cargo/config.toml" ]] && grep -q "target-dir" "$HOME/.cargo/config.toml"; then
  TARGET_DIR=$(grep "target-dir" "$HOME/.cargo/config.toml" | sed -E 's/.*=\s*"(.*)"/\1/')
  BIN_SRC="${TARGET_DIR}/release/cyberwatch"
else
  BIN_SRC="target/release/cyberwatch"
fi

if [[ ! -f "$BIN_SRC" ]]; then
  echo "Could not find built binary at: $BIN_SRC"
  exit 1
fi

case "$(uname -m)" in
  x86_64 | amd64) ARCH_DIR="linux-x86_64" ;;
  aarch64 | arm64) ARCH_DIR="linux-aarch64" ;;
  *) ARCH_DIR="linux-$(uname -m)" ;;
esac

PLUGIN_BIN_DIR="$ROOT/bin/$ARCH_DIR"
mkdir -p "$PLUGIN_BIN_DIR"
strip -o "$PLUGIN_BIN_DIR/cyberwatch" "$BIN_SRC" 2>/dev/null \
  || cp "$BIN_SRC" "$PLUGIN_BIN_DIR/cyberwatch"
chmod +x "$PLUGIN_BIN_DIR/cyberwatch"
echo "Bundled plugin binary → $PLUGIN_BIN_DIR/cyberwatch"

if [[ "${1:-}" == "--local-bin" ]]; then
  mkdir -p "$HOME/.local/bin"
  cp "$PLUGIN_BIN_DIR/cyberwatch" "$HOME/.local/bin/cyberwatch"
  chmod +x "$HOME/.local/bin/cyberwatch"
  echo "Also installed → $HOME/.local/bin/cyberwatch"
fi

# Real copies, not symlinks: `omarchy plugin validate` explicitly rejects
# symlinks inside a plugin folder, so this reruns on every install instead
# (cheap — these files are tiny) rather than linking once and drifting out
# of validation.
OMARCHY_PLUGIN_DIR="$HOME/.config/omarchy/plugins/custom.cyberwatch"
mkdir -p "$OMARCHY_PLUGIN_DIR/bin"
cp "$ROOT/manifest.json" "$OMARCHY_PLUGIN_DIR/manifest.json"
cp "$ROOT/BarWidget.qml" "$OMARCHY_PLUGIN_DIR/BarWidget.qml"
cp "$ROOT/cyberwatch-toggle" "$OMARCHY_PLUGIN_DIR/cyberwatch-toggle"
chmod +x "$OMARCHY_PLUGIN_DIR/cyberwatch-toggle"
cp -r "$ROOT/bin/." "$OMARCHY_PLUGIN_DIR/bin/"
echo "Copied into Omarchy bar plugins → $OMARCHY_PLUGIN_DIR"
echo
echo "Not yet enabled in the bar — run:"
echo "  omarchy plugin enable custom.cyberwatch --section right"
