# Feature Dependencies Generator

## Purpose

`bazel/feature_deps` contains a small Rust command-line tool plus Bazel rules that
generate checked-in `.bzl` files from a crate's `[features]` section in
`Cargo.toml`.

The generated `.bzl` file serves two purposes:

1. It exposes `*_DEPS` constants listing the Bazel dependency labels pulled in by
   a feature.
2. It exposes `*_FEATURES` constants listing the transitive Rust feature names
   pulled in by a feature.

This is currently used by `terminal/Cargo.toml` and
`terminal/terminal_features.bzl`.

## Cargo.toml Semantics

Only the `[features]` section is relevant.

For a feature entry string:

- If it starts with `dep:`, it represents a dependency activated by that
  feature.
- If it contains `/`, it activates a feature on a dependency and is ignored by
  this generator.
- Otherwise, it references another feature in the same crate and should be
  traversed recursively.

## Command-Line Tool

The Rust executable is the `feature-deps` crate.

It accepts:

- positional `cargo_toml`: path to an existing `Cargo.toml`
- positional `output_bzl`: path to the `.bzl` file to create
- repeated `--dependency-alias DEP=LABEL`: rewrites a `dep:DEP` entry to a
  specific Bazel label instead of the default `@crates//:DEP`
- repeated `--dependency-exclusion DEP`: omits a `dep:DEP` entry from generated
  `*_DEPS` constants entirely

Parsing is implemented with `clap`.

## Traversal Algorithm

For each feature:

1. Keep a visited set so each feature is emitted once.
2. DFS through same-crate child features before emitting the current feature.
3. Track:
   - `child_features`: sorted set of referenced same-crate features
   - `dependencies`: sorted set of Bazel labels generated from `dep:` entries
4. Ignore dependency feature activations such as `server_fn/browser`.

Dependency label handling is:

- If a dependency appears in `dependency_exclusion`, do not emit it.
- Otherwise, if it appears in `dependency_aliases`, emit the aliased Bazel
  label.
- Otherwise emit `@crates//:{dependency}`.

## Generated Constants

Each feature emits two constants:

- `{FEATURE_NAME}_DEPS`
- `{FEATURE_NAME}_FEATURES`

Both names use `feature_name.to_shouty_snake_case()`.

### `*_DEPS`

`*_DEPS` contains the transitive dependency labels pulled by a feature.

Ordering:

- Child feature `*_DEPS` constants come first.
- The current feature's direct dependency labels come after.

Example:

```bzl
CLIENT_DEPS = ["@crates//:stylance"]
TERMINAL_DEPS = CLIENT_DEPS + ["@crates//:scopeguard"]
```

### `*_FEATURES`

`*_FEATURES` contains the transitive Rust feature names pulled by a feature.

Ordering:

- The current feature name comes first as a one-element list.
- Child feature `*_FEATURES` constants come after.

Example:

```bzl
CORRELATION_ID_FEATURES = ["correlation-id"]
TERMINAL_FEATURES = ["terminal"] + CORRELATION_ID_FEATURES
CLIENT_FEATURES = ["client"]
TERMINAL_CLIENT_FEATURES = ["terminal-client"] + CLIENT_FEATURES + TERMINAL_FEATURES
```

## Bazel Packaging

The Bazel API is defined in `bazel/feature_deps/defs.bzl`.

### `feature_deps_tool()`

Builds the `feature-deps` Rust binary.

### `feature_deps(...)`

Generates a checked-in `{name}-features.bzl` file from a `Cargo.toml`.

Parameters:

- `name`: defaults to the current package basename
- `path`: defaults to `Cargo.toml`
- `dependency_aliases`: optional `dict[str, str]` mapping `dep:` entries to
  Bazel labels
- `dependency_exclusion`: optional `list[str]` of `dep:` entries to omit from
  generated `*_DEPS`

Behavior:

1. The custom rule invokes the `feature-deps` binary and writes
   `generated.{name}-features.bzl`.
2. The `generate_file` helper copies that generated file into the checked-in
   `{name}-features.bzl` when run via `bazel run //pkg:{name}_update`.

The checked-in `.bzl` file is still required because Bazel cannot `load()` a
Starlark file produced by a normal build action in the same loading phase.

## Current Example: `terminal`

`terminal/BUILD.bazel` currently uses:

- generated `*_DEPS` constants to populate `client_deps` and `server_deps`
- generated `*_FEATURES` constants to populate `client_features` and
  `server_features`

`terminal` also configures:

- dependency aliases for:
  - `terrazzo-pty -> //pty`
  - `trz-gateway-client -> //remote/client`
  - `trz-gateway-common -> //remote/common`
- dependency exclusion for:
  - `trz-gateway-server`

That exclusion exists so `terminal/BUILD.bazel` can choose whether to depend on
`//remote/server` or `//remote/server:acme` explicitly.

## Validation

Useful validation commands:

- `cargo test -p feature-deps`
- `cargo +nightly fmt`
- `bazel run //terminal:terminal_update`
- `bazel run //bazel:buildifier`
- `bazel run //bazel:buildifier_check`
- `bazel test //terminal/...`
