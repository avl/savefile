#!/usr/bin/env bash
# Runs the full workspace test suite on a big-endian target (s390x),
# using `cross` with qemu-user inside a container.
#
# Requires `cross` (cargo install cross) and podman or docker.
# Any extra arguments are passed on to `cross test`, e.g.:
#   ./test_big_endian.sh -p savefile-test test_enum
set -euo pipefail

cd "$(dirname "$0")"

if [ -z "${CROSS_CONTAINER_ENGINE:-}" ]; then
    if command -v podman > /dev/null; then
        export CROSS_CONTAINER_ENGINE=podman
    elif command -v docker > /dev/null; then
        export CROSS_CONTAINER_ENGINE=docker
    else
        echo "Neither podman nor docker found" >&2
        exit 1
    fi
fi

if [ $# -eq 0 ]; then
    set -- --workspace
fi

exec cross test --no-fail-fast --target s390x-unknown-linux-gnu "$@"
