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

# Figure out if we are using docker or podman or need to provide some
# installation guidance

which docker > /dev/null 2>&1 || which podman > /dev/null 2>&1 || advise "${install_conman_advice:?}"
which cross > /dev/null 2>&1  || advise "${install_cross_advice:?}"

while IFS=":" read -r title program schema query; do
    [[ "${title}" =~ ^#.* ]] && continue
    ./scripts/run_test.sh "${title}" "${program}" "${schema}" "${query}" || terminate "finishing batch because ${title} failed"
done < testdata/controlfile
