#!/bin/sh
# Runs the measurement mode of the client on Linux: a release build, a
# daemon in a temporary HOME with one shell session, Xvfb as the display.
# Prints the report (JSON). The scenario takes about 90 seconds.
#
# Needs: the nomux binaries of the pinned revision on PATH, Xvfb and a
# Vulkan driver. In the development container:
#   tools/dev-container/run.sh tools/measure.sh
set -eu

cargo build --release --locked -p nomux-desktop
client=$PWD/target/release/nomux-desktop
report=$PWD/target/measure.json

home=$(mktemp -d)
xvfb=
trap 'nomux daemon stop >/dev/null 2>&1 || true; [ -n "$xvfb" ] && kill "$xvfb"; rm -rf "$home"' EXIT
export HOME="$home" XDG_RUNTIME_DIR="$home/run"
mkdir -m 700 "$XDG_RUNTIME_DIR"
nomux new -- bash --norc >/dev/null

Xvfb :71 -screen 0 1280x800x24 >/dev/null 2>&1 &
xvfb=$!
sleep 1
DISPLAY=:71 NOMUX_DESKTOP_LAUNCH_NS=$(date +%s%N) "$client" --measure "$report"
cat "$report"
