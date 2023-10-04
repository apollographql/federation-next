# README

`apollo-harness` provides a mechanism for running different query planning implementations and extracting interesting performance data.

## Supported Platforms
 - macOS m1/m2
 - linux x86

# Getting Started

Clone the `federation-next` repo and cd to `apollo-harness`. Some of the scripts make assumptions that you are in this directory, so don't cd away from here.

You'll find there are several sub-directories:

## scripts
The scripts you'll need to run a batch of tests are here.

The only one you will be running is `scripts/run_tests.sh`. This script checks that your execution environment looks ok and provides some advice on installing/configuring required tooling before executing a batch of tests.

## src

This directory contains the source code for building a rust executable which performs schema loading and (optionally) query planning. The only binary right now is `rb_loader` which uses the `router-bridge` to perform these tasks.

When you execute a batch of tests, this code is cross-compiled into a linux container, so that heaptrack can be reliably executed from your host.

You shouldn't need to make any changes to this source code unless you are adding a new federation schema loading query planner. In which case, add a new `[[bin]]` and implement accordingly.

## testdata

This directory contains a `controlfile`. Any line beginning with `#` is ignored and the format is documented in the file. For example:

```
# Format -> free text title:program name:schema file:query file
load and plan 1:rb_loader:schema.graphql:query.graphql
load 1:rb_loader:schema.graphql:
load 2:rb_loader:mohameds-team.graphql:
load 3:rb_loader:symbiose.graphql:
```

This controlfile will result in a batch of tests which will generate results in the results/[test name] directory. All tests will load a schema file, so the schema file argument is mandatory. You may optionally provide a query which is planned.

## results

The results of running each test in the controlfile are generated here. The full output of heaptrack analysis is captured in a compressed timestamped file alongside a summary report which has the same timestamp. For most purposes, the summary file will be enough information to determine if a performance regression has occurred, but if you are investigating a regression, you will probably want to access the full heaptrack data in the compressed data file.

# Interpreting Results

Here's what the output of a typical run looks like:
```
garypen@Garys-MacBook-Pro apollo-harness % ./scripts/run_tests.sh   
Using /usr/local/bin/docker to run the tests...
Building target: aarch64-unknown-linux-gnu
heaptrack stats:
	allocations:          	130821
	leaked allocations:   	1190
	temporary allocations:	33166

Results: load and plan 1 -> load_and_plan_1/2023_10_04_11:38:45.out
total runtime (un-instrumented): 0.15s
total runtime: 0.29s.
calls to allocation functions: 130821 (449556/s)
temporary memory allocations: 35118 (120680/s)
peak heap memory consumption: 5.87M
peak RSS (including heaptrack overhead): 62.90M
total memory leaked: 250.41K
Using /usr/local/bin/docker to run the tests...
Building target: aarch64-unknown-linux-gnu
heaptrack stats:
	allocations:          	108603
	leaked allocations:   	1190
	temporary allocations:	29346

Results: load 1 -> load_1/2023_10_04_11:38:48.out
total runtime (un-instrumented): 0.13s
total runtime: 0.22s.
calls to allocation functions: 108603 (484834/s)
temporary memory allocations: 31066 (138687/s)
peak heap memory consumption: 5.84M
peak RSS (including heaptrack overhead): 62.13M
total memory leaked: 250.41K
Using /usr/local/bin/docker to run the tests...
Building target: aarch64-unknown-linux-gnu
heaptrack stats:
	allocations:          	319121
	leaked allocations:   	1191
	temporary allocations:	39045

Results: load 2 -> load_2/2023_10_04_11:38:52.out
total runtime (un-instrumented): 2.35s
total runtime: 2.59s.
calls to allocation functions: 319121 (123307/s)
temporary memory allocations: 56847 (21965/s)
peak heap memory consumption: 7.57M
peak RSS (including heaptrack overhead): 230.15M
total memory leaked: 254.52K
Using /usr/local/bin/docker to run the tests...
Building target: aarch64-unknown-linux-gnu
heaptrack stats:
	allocations:          	348186
	leaked allocations:   	1191
	temporary allocations:	45595

Results: load 3 -> load_3/2023_10_04_11:39:00.out
total runtime (un-instrumented): 1.30s
total runtime: 1.45s.
calls to allocation functions: 348186 (240128/s)
temporary memory allocations: 63324 (43671/s)
peak heap memory consumption: 8.96M
peak RSS (including heaptrack overhead): 178.72M
total memory leaked: 254.52K
garypen@Garys-MacBook-Pro apollo-harness % 
```

Most of the data is self explanatory. For our purposes, we are mainly interested in two values:

total runtime (un-instrumented): 
peak RSS (including heaptrack overhead):

These two values tell us, the wall clock for executing the test (measure execution performance regressions) and the peak amount of resident memory (measure memory resource consumption regressions).

# TODO

1. Write a baseline comparison and check in some baseline data.
