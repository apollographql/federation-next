#! /usr/bin/env bash

###
# Run an apollo-harness test under heaptrack.
#
# Since heaptrack is linux specific, the best way to do this is by running
# the tests in a container.
###

###
# Terminate the build and clean up the build directory
###
terminate () {
    printf "%s terminating...\n" "${1}"
    exit 1
}

###
# Advise about installation/configuration and then terminate
###
advise () {
    printf "%\n" "${1}"
    exit 2
}

install_conman_advice="""
The test harness executes within a container, so your machine must provide some kind of container management facility.

We support:
 - docker
 - podman

You can install/configure them by following the instructions at:

docker
------

https://docs.docker.com/engine/install/

podman
------

linux: (Figure this out for your distro. Likely to be something like 'apt install podman')

macOS: 'brew install podman'. Decide if you are all in on podman, if you are also 'brew install podman-desktop', if not 'podman machine init && podman machine start')

Note: Install/Configuring Docker/Podman could be a fairly complex task, these directions are minimal and should be enough to get you started. There's plenty of documentation on the internet if you want to fine tune your installation.
Once docker/podman is installed, please start the test again.
"""

install_cross_advice="""
The test harness makes use of the cargo cross plugin to perform cross compiling.

You can install cross as follows:

cargo install cross --git https://github.com/cross-rs/cross

Once cross is installed, please start the test again.
"""

# Figure out if we are using docker or podman or need to provide some
# installation guidance

CONMAN=$(which docker || which podman) || advise "${install_conman_advice}"
CROSS=$(which cross) || advise "${install_cross_advice}"

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

printf "Building target: %s\n" "${TARGET}"

# Before we do any building set CROSS environment up to disable buildx.
# buildx may not exist in every environment and we don't need it.
export CROSS_CONTAINER_ENGINE_NO_BUILDKIT=1

# Before building make sure the target directory exists.
# If it doesn't, the docker container will create it as root and that breaks a lot of things
# We can ignore any fails from this command
mkdir ../target > /dev/null 2>&1

# Use cross to cross compile to desired target
${CROSS} build --release --bin "${2}" --target "${TARGET}" > /dev/null 2>&1

# Build an image to run our target
${CONMAN} build \
    -t apollo_harness:latest \
    -f scripts/Dockerfile.runner \
    scripts > /dev/null 2>&1

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
        "testdata/${4}"
else
    ${CONMAN} run \
        --rm \
        --mount "type=bind,source=${PWD}/scripts,target=/scripts" \
        --mount "type=bind,source=${PWD}/results,target=/results" \
        --mount "type=bind,source=${PWD}/testdata,target=/testdata" \
        --mount "type=bind,source=${PWD}/../target/${TARGET}/release,target=/programs" \
        apollo_harness:latest /scripts/runit.sh "${timestamp}" \
        "${2}" \
        "testdata/${3}"
fi

# Display the heaptrack analyze results
printf "\nResults: %s -> %s.out\n" "${1}" "${timestamp}"
cat "results/${timestamp}.out"

