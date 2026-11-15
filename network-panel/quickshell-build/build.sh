#!/bin/bash
# Build Quickshell in a trixie container and drop the binary in ./out/.
# Used to produce the quickshell-aarch64 release asset; install.sh builds
# natively with build-quickshell.sh when no release is published.
# Usage: ./build.sh [quickshell-git-ref]
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"

REF="${1:-v0.3.2}"
IMAGE="sway-argon-quickshell:$REF"

docker build --build-arg "QUICKSHELL_REF=$REF" -t "$IMAGE" .

mkdir -p out
cid=$(docker create "$IMAGE")
trap 'docker rm -f "$cid" >/dev/null' EXIT
docker cp "$cid:/out/bin/quickshell" out/quickshell
echo "Built out/quickshell ($(du -h out/quickshell | cut -f1)) from $REF"
