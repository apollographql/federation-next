#! /usr/bin/env bash


while IFS=":" read -r title program schema query; do
    [[ "${title}" =~ ^#.* ]] && continue
    ./scripts/run_test.sh "${title}" "${program}" "${schema}" "${query}"
done < testdata/controlfile
