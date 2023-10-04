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

which docker > /dev/null 2>&1 || which podman > /dev/null 2>&1 || advise "${install_conman_advice}"
which cross> /dev/null 2>&1  || advise "${install_cross_advice}"

while IFS=":" read -r title program schema query; do
    [[ "${title}" =~ ^#.* ]] && continue
    ./scripts/run_test.sh "${title}" "${program}" "${schema}" "${query}" || terminate "finising batch because ${title} failed"
done < testdata/controlfile
