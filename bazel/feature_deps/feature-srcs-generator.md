# Feature Sources Generator

## Purpose

Extend `bazel/feature-deps` to create per-feature constants listing the Rust source files that should be compiled when a feature is active.
Refer to `bazel/feature-deps/feature-dependencies-generator.md` for prior art.

The output should expose additional `*_SRCS` constants in the generated `.bzl` file.

This solves two related problems:

- make feature-specific Rust file selection explicit in generated Bazel metadata
- keep the generated source lists aligned with the already-generated feature dependency constants
- give downstream Bazel rules a stable per-feature file list instead of requiring them to re-derive it

The generator should still run a synthetic pass for a feature named `default`.
For each feature:

- if `default` is already present in `Cargo.toml`, do not process it twice
- generate a constant named `${FEATURE}_SRCS`
- start by aggregating the `${REFERENCED_FEATURE}_SRCS` constants for features already referenced by that feature, similar to the dependency generator
- then compute the feature-local Rust sources starting from `lib.rs`
- finally concatenate the referenced-feature constants with the feature-local list

Example shape:

```starlark
CONVERTER_CLIENT_SRCS = DEFAULT_SRCS + CLIENT_SRCS + CONVERTER_SRCS + REMOTES_UI_SRCS + [
    "lib.rs",
    ...
]
```

## Improve The Algorithm

Suggested algorithm:

`aggregate_feature_srcs(feature, file_rs, included, accumulator)`

- initial call:
  - `aggregate_feature_srcs(feature, "lib.rs", false, &mut Vec<String>)`
  - for the synthetic `default` feature, start with `included = true`
- if `included` is `true`, add `file_rs` to `accumulator`
- parse `file_rs` with `syn`
- inspect only out-of-line top-level module declarations:
  - include `mod my_sub_module;`
  - ignore inline modules such as `mod my_sub_module { ... }`
- resolve the source file for each submodule using Rust module conventions
- determine whether each submodule is included for the current feature
- recurse into every discovered submodule:
  - pass `included = true` when the submodule is included for the current feature
  - pass `included = false` otherwise

Inclusion rules for a submodule:

- if the parent path is already excluded, keep traversing conservatively but do not add files unless the submodule is clearly feature-enabled
- if the `mod` statement is annotated with `#[cfg(feature = "$feature")]`, include it
- if the target file starts with `#![cfg(feature = "$feature")]`, include it
- if the guard clearly targets a different feature, exclude it
- if there is no `cfg` guard at all on the submodule declaration or submodule file, propagate the parent's `included` value
- if the predicate is composite, for example `#[cfg(any(feature = "$feature", unix))]`, be conservative and include it unless it is clearly impossible for the current feature

That conservative rule is important because the generator is producing a safe superset for Bazel source declaration. False positives are usually acceptable; false negatives are more dangerous because they can make the generated Bazel target incomplete.

## Simple Example

Assume a crate with two features, `client` and `server`, and these files:

- `lib.rs`
- `client.rs`
- `server.rs`
- `client/api.rs`
- `server/http.rs`

Example module declarations:

`lib.rs`

```rust
#[cfg(feature = "client")]
mod client;

#[cfg(feature = "server")]
mod server;
```

`client.rs`

```rust
mod api;
```

`server.rs`

```rust
#![cfg(feature = "server")]

mod http;
```

In this example:

- `lib.rs` always exists and is the traversal root
- `client.rs` is only included when `client` is active because its `mod` statement in `lib.rs` is feature-gated
- `server.rs` is only included when `server` is active because both the `mod` statement and the file-level inner attribute gate it
- `client/api.rs` is included whenever `client.rs` is included because its module declaration is unconditional
- `server/http.rs` is included whenever `server.rs` is included because its module declaration is unconditional

The generated constants would therefore look like:

```starlark
DEFAULT_SRCS = [
    "lib.rs",
]

CLIENT_SRCS = DEFAULT_SRCS + [
    "client.rs",
    "client/api.rs",
]

SERVER_SRCS = [
    "server.rs",
    "server/http.rs",
]
```

## Validation

Useful validation commands:

- `bazel run //terminal:terminal_update`
- `bazel run //bazel:buildifier`
- `bazel run //bazel:buildifier_check`
