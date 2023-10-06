#! /usr/bin/env bash

###
# Run an apollo-harness test under heaptrack.
#
# Since heaptrack is linux specific, the best way to do this is by running
# the tests in a container.
###

# shellcheck disable=SC1091
# shellcheck source=./incl.sh
source "$(dirname "${0}")/incl.sh"

CONMAN=$(which docker || which podman) || advise "${install_conman_advice:?}"
CROSS=$(which cross) || advise "${install_cross_advice:?}"

printf "Using %s to run the tests...\n" "${CONMAN}"

# Figure out our host platform. We'll use that to decide what kind of target to build
PLATFORM="$(uname -m)"

if [[ "${PLATFORM}" == "amd64" || "${PLATFORM}" == "x86_64" ]]; then
    TARGET="x86_64-unknown-linux-gnu"
elif [[ "${PLATFORM}" == "arm64" ]]; then
    TARGET="aarch64-unknown-linux-gnu"
else
    terminate "unsupported platform ${PLATFORM}"
fi

# This check makes sure that cross won't be installed with a toolchain which doesn't
# match the default for the host. This can be the source of extremely strange behaviour
# such as trying to build amd64 docker images on an arm64 system.

toolchain_arch=$(rustup toolchain list | grep default | cut -d' ' -f 1 | cut -d '-' -f 2)
target_arch=${TARGET%%-*}

if [[ "${toolchain_arch}" != "${target_arch}" ]]; then
    terminate "please use 'rustup default <toolchain>' to set your default toolchain to use a stable toolchain with arch: ${target_arch} before re-trying"
fi

printf "Building target: %s\n" "${TARGET}"

# Before we do any building set CROSS environment up to disable buildx.
# buildx may not exist in every environment and we don't need it.
export CROSS_CONTAINER_ENGINE_NO_BUILDKIT=1

# Before building make sure the target directory exists.
# If it doesn't, the docker container will create it as root and that breaks a lot of things
# We can ignore any fails from this command
mkdir ../target/"${TARGET}"/release > /dev/null 2>&1

# Use cross to cross compile to desired target
${CROSS} build --release --bin "${2}" --target "${TARGET}" > /dev/null 2>&1 || terminate "${CROSS} failed to build our test executable"

# Build an image to run our target
${CONMAN} build \
    -t apollo_harness:latest \
    -f scripts/Dockerfile.runner \
    scripts > /dev/null 2>&1 || terminate "${CONMAN} failed to build our heaptrack execution container"

# Create a timestamped filename for our test
timestamp="${1// /_}/$(date +'%Y_%m_%d_%H:%M:%S')"

# Run the test with 1 or 2 arguments
if [[ "${4}" != "" ]]; then
    ${CONMAN} run \
        --rm \
        --mount "type=bind,source=${PWD}/scripts,target=/scripts" \
        --mount "type=bind,source=${PWD}/results,target=/results" \
        --mount "type=bind,source=${PWD}/testdata,target=/testdata" \
        --mount "type=bind,source=${PWD}/../target/${TARGET}/release,target=/programs" \
        apollo_harness:latest /scripts/runit.sh "${timestamp}" \
        "${2}" \
        "testdata/${3}" \
        "testdata/${4}" > /dev/null 2>&1 || terminate "${CONMAN} failed to execute our test under heaptrack"
else
    ${CONMAN} run \
        --rm \
        --mount "type=bind,source=${PWD}/scripts,target=/scripts" \
        --mount "type=bind,source=${PWD}/results,target=/results" \
        --mount "type=bind,source=${PWD}/testdata,target=/testdata" \
        --mount "type=bind,source=${PWD}/../target/${TARGET}/release,target=/programs" \
        apollo_harness:latest /scripts/runit.sh "${timestamp}" \
        "${2}" \
        "testdata/${3}" > /dev/null 2>&1 || terminate "${CONMAN} failed to execute our test under heaptrack"
fi

# Display the heaptrack analyze results
printf "\nResults: %s -> %s.out\n" "${1}" "${timestamp}"
cat "results/${timestamp}.out"
echo

