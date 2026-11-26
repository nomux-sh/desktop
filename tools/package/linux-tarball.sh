#!/bin/sh
# Makes the Linux archive: the client, the three binaries of the nomux core,
# the README and the licenses in one directory.
#
# Usage: tools/package/linux-tarball.sh <target> <directory with nomux, nomuxd and nomux-holder>
# Output: target/dist/nomux-desktop-<version>-<target>.tar.gz
set -eu

target=$1
core=$2

version=$(tools/release/workspace-version.sh)
name=nomux-desktop-$version-$target
staging=$(mktemp -d)
mkdir -p "$staging/$name" target/dist

cp "target/$target/release/nomux-desktop" "$staging/$name/"
for binary in nomux nomuxd nomux-holder; do
  cp "$core/$binary" "$staging/$name/"
done
cp README.md LICENSE-APACHE LICENSE-MIT NOTICE "$staging/$name/"

archive=target/dist/$name.tar.gz
tar -czf "$archive" -C "$staging" "$name"
rm -rf "$staging"
echo "$archive"
