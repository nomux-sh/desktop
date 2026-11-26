#!/bin/sh
# Runs the measurement mode of the client on Linux: a release build, a
# daemon in a temporary HOME with one shell session, and a display without
# a screen: Xvfb (x11, the default) or a headless Weston (wayland). Prints
# the report (JSON). The scenario takes about two minutes.
#
# Needs: the nomux binaries of the pinned revision on PATH, Xvfb or Weston
# and a Vulkan driver. In the development container:
#   tools/dev-container/run.sh tools/measure.sh [x11|wayland]
set -eu

backend=${1:-x11}

cargo build --release --locked -p nomux-desktop
client=$PWD/target/release/nomux-desktop
report=$PWD/target/measure-$backend.json

home=$(mktemp -d)
display_server=
trap 'nomux daemon stop >/dev/null 2>&1 || true; [ -n "$display_server" ] && kill "$display_server"; rm -rf "$home"' EXIT
export HOME="$home" XDG_RUNTIME_DIR="$home/run"
mkdir -m 700 "$XDG_RUNTIME_DIR"
nomux new -- bash --norc >/dev/null

case "$backend" in
  x11)
    Xvfb :71 -screen 0 1280x800x24 >/dev/null 2>&1 &
    display_server=$!
    export DISPLAY=:71
    ;;
  wayland)
    # GPUI needs a seat: the headless backend has none without --fake-seat.
    weston --backend=headless --fake-seat --width=1280 --height=800 \
      --socket=wayland-71 >/dev/null 2>&1 &
    display_server=$!
    unset DISPLAY
    export WAYLAND_DISPLAY=wayland-71
    ;;
  *)
    echo "measure.sh: the backend is x11 or wayland" >&2
    exit 1
    ;;
esac
sleep 1
NOMUX_DESKTOP_LAUNCH_NS=$(date +%s%N) "$client" --measure "$report"
cat "$report"
