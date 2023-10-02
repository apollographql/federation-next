#! /usr/bin/env bash

timestamp="${1}";

heaptrack -o /results/"${timestamp}" /programs/apollo-harness "${2}" "${3}" > /dev/null

timed="$(/usr/bin/time -f '%e' /programs/apollo-harness "${2}" "${3}" 2>&1 > /dev/null)"

printf "total runtime (un-instrumented): %ss\n" "${timed}" > /results/"${timestamp}.out"

heaptrack --analyze "/results/${timestamp}.gz" | tail -6 >> /results/"${timestamp}.out"
