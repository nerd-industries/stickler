#!/usr/bin/env bash
# Build and install Stickler (desktop app + MCP server) for the current user.
set -euo pipefail
cd "$(dirname "$0")/.."

BIN="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor"

echo "Building the app…"
(cd app/ui && npm ci --silent)
(cd app/src-tauri && cargo tauri build --no-bundle)
echo "Building the MCP server…"
cargo build --release -p nelko-mcp

mkdir -p "$BIN" "$APPS" "$ICONS/128x128/apps" "$ICONS/256x256/apps"
install -m 755 target/release/stickler "$BIN/stickler"
install -m 755 target/release/stickler-mcp "$BIN/stickler-mcp"
install -m 644 app/src-tauri/icons/128x128.png "$ICONS/128x128/apps/stickler.png"
install -m 644 app/src-tauri/icons/128x128@2x.png "$ICONS/256x256/apps/stickler.png"
install -m 644 packaging/stickler.desktop "$APPS/stickler.desktop"
update-desktop-database "$APPS" 2>/dev/null || true
gtk-update-icon-cache -q "$ICONS" 2>/dev/null || true

if command -v claude >/dev/null; then
  # Re-point any existing registration at the installed binary.
  claude mcp remove --scope user stickler >/dev/null 2>&1 || true
  claude mcp add --scope user stickler -- "$BIN/stickler-mcp"
  echo "Registered the 'stickler' MCP server with Claude Code."
fi

echo "Done. Launch “Stickler” from your app menu, or run: stickler"
