#!/bin/bash
# Build cece, install it to ~/.local/bin and register the Omarchy bar widget.
#
#   ./install.sh              build + install binary + link and enable plugin
#   ./install.sh --no-plugin  only build and install the binary
#   ./install.sh --uninstall  stop the cat, remove the binary and the plugin

set -euo pipefail

cd "$(dirname "$(readlink -f "$0")")"

BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
PLUGIN_ID="ure.cece"
PLUGIN_DIR="$HOME/.config/omarchy/plugins/$PLUGIN_ID"

plugin=1
case "${1:-}" in
  --no-plugin) plugin=0 ;;
  --uninstall)
    pkill -x cece || true
    if command -v omarchy >/dev/null; then
      omarchy plugin disable "$PLUGIN_ID" 2>/dev/null || true
    fi
    if [[ -f $PLUGIN_DIR/manifest.json && ! -d $PLUGIN_DIR/.git ]]; then
      rm -r "$PLUGIN_DIR"
    fi
    rm -f "$BIN_DIR/cece"
    echo "cece uninstalled"
    exit 0
    ;;
  "") ;;
  *)
    echo "usage: $0 [--no-plugin|--uninstall]" >&2
    exit 1
    ;;
esac

cargo build --release
mkdir -p "$BIN_DIR"
install -m 755 target/release/cece "$BIN_DIR/cece"
echo "installed $BIN_DIR/cece"

(( plugin )) || exit 0
if ! command -v omarchy >/dev/null; then
  echo "omarchy not found; skipping the bar widget. Start the cat with: cece &"
  exit 0
fi

# A checkout installed with `omarchy plugin add` already is the plugin
# directory. Anywhere else, copy the widget in (Omarchy refuses symlinks).
if [[ $(readlink -f "$PLUGIN_DIR" 2>/dev/null) != "$PWD" ]]; then
  mkdir -p "$PLUGIN_DIR"
  install -m 644 manifest.json BarWidget.qml LICENSE "$PLUGIN_DIR/"
  echo "copied the bar widget to $PLUGIN_DIR"
fi
omarchy plugin validate "$PLUGIN_DIR"
omarchy-shell shell rescanPlugins >/dev/null 2>&1 || true
# The rescan finishes asynchronously; give the shell a moment to see the plugin.
enabled=0
for _ in 1 2 3 4 5 6 7 8 9 10; do
  if omarchy plugin enable "$PLUGIN_ID" --section right 2>/dev/null; then
    enabled=1
    break
  fi
  sleep 0.5
done
(( enabled )) || { echo "could not enable $PLUGIN_ID; try: omarchy plugin enable $PLUGIN_ID" >&2; exit 1; }
echo "Cece is in your bar: click to mute, right-click to stay, middle-click to send it away."
