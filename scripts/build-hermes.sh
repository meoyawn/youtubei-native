#!/bin/sh
set -eu

crate_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
hermes_source=${HERMES_SOURCE_DIR:-"$crate_root/.hermes/source"}
hermes_build=${HERMES_BUILD_DIR:-"$crate_root/.hermes/build"}
revision=7508017ae267ecffe4c4df38656713034f35d9bf

if [ ! -d "$hermes_source/.git" ]; then
    mkdir -p "$hermes_source"
    git -C "$hermes_source" init
    git -C "$hermes_source" remote add origin https://github.com/facebook/hermes.git
    git -C "$hermes_source" fetch --depth=1 origin "$revision"
    git -C "$hermes_source" checkout --detach FETCH_HEAD
fi

if [ "$(git -C "$hermes_source" rev-parse HEAD)" != "$revision" ]; then
    echo "Hermes checkout does not match the pinned ABI revision" >&2
    exit 1
fi

cmake -S "$hermes_source" -B "$hermes_build" \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
    -DHERMES_ENABLE_TEST_SUITE=OFF -DHERMES_ENABLE_INTL=OFF
cmake --build "$hermes_build" --target shermes shermes-dep -j "${BUILD_JOBS:-4}"
