#! /usr/bin/env bash

timestamp="${1}";
program="${2}";

if [[ ! -z "${4}" ]];then
    heaptrack -o /results/"${timestamp}" /programs/"${program}" "${3}" "${4}" > /dev/null

    timed="$(/usr/bin/time -f '%e' /programs/"${program}" "${3}" "${4}" 2>&1 > /dev/null)"
else
    heaptrack -o /results/"${timestamp}" /programs/"${program}" "${3}" > /dev/null

    timed="$(/usr/bin/time -f '%e' /programs/"${program}" "${3}" 2>&1 > /dev/null)"
fi

printf "total runtime (un-instrumented): %ss\n" "${timed}" > /results/"${timestamp}.out"

heaptrack --analyze "/results/${timestamp}.gz" | tail -6 >> /results/"${timestamp}.out"
