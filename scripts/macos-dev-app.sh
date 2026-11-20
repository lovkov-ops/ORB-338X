#!/usr/bin/env bash
# Wraps the UI you just built into a local ORB-338X.app for everyday runs (task-110), so the
# Dock and Activity Monitor show the app icon (an unbundled binary has none), without building
# the .dmg:
#
#   cargo build --release -p orb-ui && scripts/macos-dev-app.sh && open target/ORB-338X.app
#
#   scripts/macos-dev-app.sh [UI_BINARY]
#
# UI_BINARY defaults to target/release/ORB-338X. The app is target/ORB-338X.app (a copy of
# the binary, ad-hoc signed; run the script again after each build). To pass arguments or see
# the output, run the binary inside: target/ORB-338X.app/Contents/MacOS/ORB-338X SONG.rbs
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
ui=${1:-${CARGO_TARGET_DIR:-$root/target}/release/ORB-338X}
[ -f "$ui" ] || { echo "no $ui: cargo build --release -p orb-ui" >&2; exit 1; }
exec "$root/scripts/macos-app.sh" "${CARGO_TARGET_DIR:-$root/target}" "$ui"
