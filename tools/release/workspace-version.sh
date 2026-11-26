#!/bin/sh
# Prints the version of the workspace: the version of the nomux-desktop crate.
#
# Usage: tools/release/workspace-version.sh
set -eu

cd "$(git rev-parse --show-toplevel)"

cargo metadata --no-deps --format-version 1 |
  jq -r '.packages[] | select(.name == "nomux-desktop") | .version'
