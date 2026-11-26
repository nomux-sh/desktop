#!/bin/sh
# Makes nomux.app and its disk image for one macOS target, from the release
# binary of the client and the three binaries of the nomux core. Runs on
# macOS.
#
# The core binaries go next to the client in Contents/MacOS: the client
# starts the nomuxd that is in the directory of its own binary, and nomuxd
# finds nomux-holder in the same way.
#
# Usage: tools/package/macos-app.sh <target> <directory with nomux, nomuxd and nomux-holder>
# Output: target/dist/nomux-<version>-macos-<arm64|x86_64>.dmg
set -eu

target=$1
core=$2

case "$target" in
  aarch64-apple-darwin) architecture=arm64 ;;
  x86_64-apple-darwin) architecture=x86_64 ;;
  *)
    echo "macos-app.sh: unknown target $target" >&2
    exit 1
    ;;
esac

version=$(tools/release/workspace-version.sh)
client=target/$target/release/nomux-desktop
staging=$(mktemp -d)
app=$staging/nomux.app
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" target/dist

cp "$client" "$app/Contents/MacOS/nomux-desktop"
for name in nomux nomuxd nomux-holder; do
  cp "$core/$name" "$app/Contents/MacOS/$name"
done

# The minimum macOS version is the one that the linker wrote into the client.
minimum=$(vtool -show-build "$client" | awk '$1 == "minos" { print $2; exit }')
test -n "$minimum"
sed -e "s/@VERSION@/$version/g" -e "s/@MINIMUM_SYSTEM_VERSION@/$minimum/g" \
  tools/package/Info.plist >"$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"
tools/package/render-icon.sh "$app/Contents/Resources/nomux.icns"

# Ad-hoc signature: Apple silicon does not run code without a signature.
# The helpers first, then the bundle, which signs its main binary.
for name in nomux nomuxd nomux-holder; do
  codesign --force --sign - --timestamp=none --identifier "sh.nomux.desktop.$name" \
    "$app/Contents/MacOS/$name"
done
codesign --force --sign - --timestamp=none "$app"
codesign --verify --strict --verbose=2 "$app"

ln -s /Applications "$staging/Applications"
image=target/dist/nomux-$version-macos-$architecture.dmg
hdiutil create -volname "nomux $version" -srcfolder "$staging" -fs HFS+ -format UDZO -ov "$image"
hdiutil verify "$image"
rm -rf "$staging"
echo "$image"
