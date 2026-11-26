#!/bin/sh
# Prints the release notes of one version: the section of the version in
# CHANGELOG.md, then the section "First run on macOS" of README.md.
#
# Usage: tools/release/release-notes.sh <version>, for example 0.1.0.
set -eu

version=$1

cd "$(git rev-parse --show-toplevel)"

section=$(awk -v heading="## [$version]" '
  index($0, heading) == 1 { found = 1; next }
  found && /^## \[/ { exit }
  found { print }
' CHANGELOG.md)
if [ -z "$(printf '%s' "$section" | tr -d '[:space:]')" ]; then
  echo "error: CHANGELOG.md has no section for version $version." >&2
  exit 1
fi

first_run=$(awk '
  /^### First run on macOS$/ { found = 1; print; next }
  found && /^##/ { exit }
  found { print }
' README.md)
if [ -z "$first_run" ]; then
  echo "error: README.md has no section \"First run on macOS\"." >&2
  exit 1
fi

printf '%s\n\n%s\n' "$section" "$first_run"
