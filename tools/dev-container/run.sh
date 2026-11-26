#!/bin/sh
# Runs a command in the development container. Builds the image first if it
# does not exist. The repository is mounted at /work, and the cargo registry
# of the user is shared with the container.
#
# Usage: tools/dev-container/run.sh <command> [arguments]
# Example: tools/dev-container/run.sh cargo build
set -eu

repository=$(cd "$(dirname "$0")/../.." && pwd)
toolchain=$(sed -n 's/^channel = "\(.*\)"$/\1/p' "$repository/rust-toolchain.toml")
image="nomux-desktop-dev:$toolchain"
cargo_home=${CARGO_HOME:-$HOME/.cargo}

if ! podman image exists "$image"; then
  podman build --build-arg "TOOLCHAIN=$toolchain" --tag "$image" "$repository/tools/dev-container"
fi

mkdir -p "$cargo_home/registry" "$cargo_home/git"
tty_flag=
[ -t 0 ] && tty_flag=--tty

exec podman run --rm --interactive $tty_flag \
  --userns=keep-id --security-opt label=disable \
  --volume "$repository:/work" \
  --volume "$cargo_home/registry:/opt/cargo/registry" \
  --volume "$cargo_home/git:/opt/cargo/git" \
  --workdir /work \
  --env HOME=/tmp \
  --env CARGO_TERM_COLOR \
  "$image" "$@"
