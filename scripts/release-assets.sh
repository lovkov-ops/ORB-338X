#!/usr/bin/env bash
# Checks the packages of a release and writes SHA256SUMS.txt over them (task-113):
#
#   scripts/release-assets.sh DIR v2026.1028.1930
#
# DIR holds the downloaded artifacts of release.yml, flat. The 12 files that a release has
# must all be there, and nothing else (a missing package fails the release instead of
# shipping without it):
#   ORB-338X-<version>-<target>.tar.gz   x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu,
#                                          x86_64-apple-darwin, aarch64-apple-darwin
#   ORB-338X-<version>-<target>.zip      x86_64-pc-windows-msvc, aarch64-pc-windows-msvc
#   ORB-338X-<version>-<target>.dmg     x86_64-apple-darwin, aarch64-apple-darwin
#   ORB-338X-<version>-<x64|arm64>-setup.exe
#   ORB-338X-<version>-<x86_64|aarch64>.AppImage
# This fork: the six installers then get names without the version, the same in every release
# (STABLE below), so https://github.com/lovkov-ops/ORB-338X/releases/latest/download/<name>
# always gives the latest one (the website's download buttons). The archives keep theirs.
# SHA256SUMS.txt lists them by name, sorted, in the format of `sha256sum -c`.
set -euo pipefail

dir=${1:?usage: scripts/release-assets.sh DIR TAG}
tag=${2:?usage: scripts/release-assets.sh DIR TAG}
[[ $tag =~ ^v[0-9]{4}\.[0-9]{4}\.[0-9]{4}$ ]] || {
    echo "not a release tag: $tag" >&2
    exit 1
}
version=${tag#v}
p=ORB-338X-$version

expected=(
    "$p-x86_64-unknown-linux-gnu.tar.gz"
    "$p-aarch64-unknown-linux-gnu.tar.gz"
    "$p-x86_64-apple-darwin.tar.gz"
    "$p-aarch64-apple-darwin.tar.gz"
    "$p-x86_64-pc-windows-msvc.zip"
    "$p-aarch64-pc-windows-msvc.zip"
    "$p-x86_64-apple-darwin.dmg"
    "$p-aarch64-apple-darwin.dmg"
    "$p-x64-setup.exe"
    "$p-arm64-setup.exe"
    "$p-x86_64.AppImage"
    "$p-aarch64.AppImage"
)

fail=0
for f in "${expected[@]}"; do
    [ -f "$dir/$f" ] || {
        echo "missing: $f" >&2
        fail=1
    }
done
# anything else in DIR (an old SHA256SUMS.txt aside) is a package of another version
for f in "$dir"/*; do
    name=${f##*/}
    [ "$name" = SHA256SUMS.txt ] && continue
    found=0
    for e in "${expected[@]}"; do [ "$name" = "$e" ] && found=1; done
    [ $found = 1 ] || {
        echo "unexpected: $name" >&2
        fail=1
    }
done
[ $fail = 0 ] || exit 1

# the installers' names on the release, by their names from the build
stable=(
    "$p-aarch64-apple-darwin.dmg" ORB-338X-install-macOS-AppleSilicon.dmg
    "$p-x86_64-apple-darwin.dmg" ORB-338X-install-macOS-Intel.dmg
    "$p-x64-setup.exe" ORB-338X-install-Windows-x64.exe
    "$p-arm64-setup.exe" ORB-338X-install-Windows-ARM64.exe
    "$p-x86_64.AppImage" ORB-338X-Linux-x86_64.AppImage
    "$p-aarch64.AppImage" ORB-338X-Linux-ARM64.AppImage
)
for ((i = 0; i < ${#stable[@]}; i += 2)); do
    mv "$dir/${stable[i]}" "$dir/${stable[i + 1]}"
    for j in "${!expected[@]}"; do
        [ "${expected[j]}" = "${stable[i]}" ] && expected[j]=${stable[i + 1]}
    done
done

if command -v sha256sum >/dev/null; then
    sum() { sha256sum "$@"; }
else
    sum() { shasum -a 256 "$@"; }
fi
cd "$dir"
sorted=$(printf '%s\n' "${expected[@]}" | LC_ALL=C sort)
: >SHA256SUMS.txt
while read -r f; do sum "$f" >>SHA256SUMS.txt; done <<<"$sorted"
echo "wrote $dir/SHA256SUMS.txt: $(wc -l <SHA256SUMS.txt | tr -d ' ') files"
