# Feature Sources Generator

## Purpose

Extend `bazel/feature-deps` to create per-feature constant of the list of files that should be compiled when a feature is active.
Refer to bazel/feature-deps/feature-dependencies-generator.md for prior art.

It will then expose additional `*_SRCS` constants.

First of all the algorithm should be executed on a feature called "default".
For each feature:
- If the "default" feature is indeed in the list of features in Cargo.toml, no need to process it again.
- The algorithm creates a constant in the .bzl file called ${FEATURE}_SRCS
- First, aggregate the other ${FEATURE}_SRCS constants of referenced features, as previously done for dependencies.e.g. "CONVERTER_CLIENT_SRCS = DEFAULT_SRCS + CLIENT_SRCS + CONVERTER_SRCS + REMOTES_UI_SRCS"
- Then, start with file lib.rs and aggregate the list of source files as per algorithm explained below

Algorithm aggregate_feature_srcs($feature, $file.rs, $included, $accumulator):
  - The first call to aggregate_feature_srcs is with ($feature, "lib.rs", false, empty `&mut Vec<String>`), unless feature is "default", then start with $included = true
  - If $included is `true`, add $file.rs (source path from crate root) to $accumulator
  - Parse given $file.rs with the syn crate
  - Look for `mod` statements for non-nested sub-modules (not `mod my_sub_module { ... }` but `mod my_sub_module;`)
  - Determine if the sub-module is included:
    - if the sub-module statement is guarded on the feature, meaning annotated with `#[cfg(feature = "$feature")] mod my_module;` or the sub-module file starts with `#![cfg(feature = "$feature")]`, that sub-module source file will be included.
    - note that the `cfg!` predicate can be composite (like `#[cfg(any(feature = "$feature", ...))]`): in this case, be conservative, include the source file unless it is clearly gated under a different feature `#![cfg(feature = "$other_feature")]`
    - if the sub-module statement is guarded under a different feature, the file is not included
    - if the sub-module statement is not guarded, the file is included
  - process recursively all the sub-modules
    - if the sub-module is included, call aggregate_feature_srcs($feature, $file.rs, true, $accumulator)
    - else, call aggregate_feature_srcs($feature, $file.rs, false, $accumulator)

Finally, concatenate ${FEATURE}_SRCS with the computed list of src files.

## Validation

Useful validation commands:

- `bazel run //terminal:terminal_update`
- `bazel run //bazel:buildifier`
- `bazel run //bazel:buildifier_check`
