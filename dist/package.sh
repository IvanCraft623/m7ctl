#!/bin/sh
# Build a release archive in the current directory.
#   dist/package.sh <name> <version> <binary> <tar.gz|zip>
# e.g. dist/package.sh linux-x86_64 v0.1.0 target/release/m7ctl tar.gz
set -eu

[ $# -eq 4 ] || { sed -n '2,4p' "$0"; exit 2; }
name=$1 version=$2 bin=$3 kind=$4
here=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
dir="m7ctl-$version-$name"

rm -rf "$dir"
mkdir "$dir"
cp "$bin" "$dir/"
cp "$here/LICENSE" "$here/README.md" "$dir/"
case "$name" in
    linux-*) cp "$here/dist/install.sh" "$here/dist/udev/70-machenike-m7.rules" "$dir/" ;;
esac
chmod +x "$dir"/m7ctl* 2>/dev/null || true

case "$kind" in
    tar.gz) tar -czf "$dir.tar.gz" "$dir" ;;
    zip)
        if command -v 7z >/dev/null 2>&1; then 7z a -tzip -bso0 "$dir.zip" "$dir"; else zip -qr "$dir.zip" "$dir"; fi ;;
    *) echo "unknown archive kind: $kind" >&2; exit 2 ;;
esac
rm -rf "$dir"
echo "$dir.$kind"
