#! /usr/bin/env bash

timestamp="${1}";
program="${2}";

# Make a directory (if required) for our results
mkdir -p "$(dirname "${timestamp}")" > /dev/null 2>&1

# Run our test twice, first under the control of heaptrack and second not
if [[ -n "${4}" ]];then
    heaptrack -o /results/"${timestamp}" /programs/"${program}" "${3}" "${4}" > /dev/null

    timed="$(/usr/bin/time -f '%e' /programs/"${program}" "${3}" "${4}" 2>&1 > /dev/null)"
else
    heaptrack -o /results/"${timestamp}" /programs/"${program}" "${3}" > /dev/null

    timed="$(/usr/bin/time -f '%e' /programs/"${program}" "${3}" 2>&1 > /dev/null)"
fi

# Output the summary data 
printf "total runtime (un-instrumented): %ss\n" "${timed}" > /results/"${timestamp}.out"

heaptrack --analyze "/results/${timestamp}.gz" | tail -6 >> /results/"${timestamp}.out"
