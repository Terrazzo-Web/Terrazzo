# Bazel helpers

This folder contains the shared Bazel rules and small scripts used by the
Terrazzo workspace. The rules here wrap third-party Bazel modules such as
`rules_rust`, `rules_rust_wasm_bindgen`, `rules_shell`, and Buildifier with the
repo-specific conventions needed by the framework, demo, terminal, and utility
crates.

The most commonly used files are:

- `rust_rules.bzl`: Rust build, test, fmt, clippy, source-mirroring, and asset
  staging helpers.
- `scss_rules.bzl`: SCSS bundling through the Terrazzo CSS CLI.
- `generated_file.bzl` and `generated_file.sh`: helpers for checked-in generated
  files.
- `playwright_rules.bzl`, `playwright_setup.sh`, and `playwright_test.sh`:
  Playwright dependency setup and browser integration test wrappers.
- `crate_aliases.bzl`, `crate_macros/`, `feature_deps/`, and
  `client_server_modules/`: generated or supporting helpers for Rust crate
  aliases, feature-aware source selection, and client/server module variants.
- `BUILD.bazel`: public tool targets such as `//bazel:buildifier`,
  `//bazel:buildifier_check`, `//bazel:playwright_setup`, and the
  wasm-bindgen toolchain registration.

## Rust rules

Load `rust_rules` when one Rust target is enough, or `rust_rules_matrix` when
the same crate needs several variants, such as client/server or debug/release
targets.

```starlark
load("//bazel:rust_rules.bzl", "rust_rules", "rust_rules_matrix")

rust_rules(
    name = "my_lib",
    deps = [
        "//framework/terrazzo:client",
        "@crates//:web-sys",
    ],
    crate_features = ["client"],
)

rust_rules(
    name = "my_tool",
    crate_root = "src/main.rs",
    rule = "binary",
    deps = ["@crates//:clap"],
)
```

By default, `rust_rules` uses all `src/**/*.rs` files, infers the Rust package
name from the Bazel package, adds the `bazel` crate feature, and resolves normal
dependencies from the crate universe. It also creates companion targets:

- `<name>-build-test`
- `<name>-test`
- `<name>-rustfmt`
- `<name>-clippy`
- `<name>-mirror`, `<name>-mirror-rs`, `<name>-mirror-data`, and
  `<name>-mirror-manifest`

The mirror targets stage `Cargo.toml`, Rust sources, and requested assets into a
Cargo-like tree under Bazel output. That lets crates still use
`CARGO_MANIFEST_DIR`, build-script-style data, and static asset paths while being
built by Bazel.

Use `assets` when a Rust target needs runtime files or generated files staged
next to the crate:

```starlark
rust_rules(
    name = "server-lib",
    assets = [
        [
            "assets/index.html",
            "assets/bootstrap.js",
        ],
        {
            "targets": [":app_scss"],
            "prefix": "target/css",
        },
        {
            "targets": [":client"],
            "prefix": "target/assets/wasm",
        },
    ],
    crate_features = ["server"],
    deps = ["//framework/terrazzo:server"],
)
```

Each asset item can be a string, a list of labels, or a map with `targets`,
optional `prefix`, and optional `copy = True`. Assets without `copy` are linked
when possible; copied assets are useful when a tool expects regular files.

Use `rust_rules_matrix` to expand related targets:

```starlark
rust_rules_matrix(
    crate_features = ["client"],
    overrides = {
        "client-lib": {
            "deps": ["//framework/terrazzo:client"],
        },
        "client-shared-lib": {
            "deps": ["//framework/terrazzo:client"],
            "generate_tests": False,
            "rule": "shared_library",
            "target_compatible_with": ["@platforms//cpu:wasm32"],
        },
    },
)
```

`overrides` maps target names to per-target attributes. Shared attributes are
passed once at the matrix level.

## SCSS rules

`scss_rule` runs `//utils/css/cli` over the package containing `Cargo.toml` and
emits one bundled SCSS file. The CLI finds SCSS/CSS modules, rewrites class names
to the hashed names imported by `terrazzo_css::import_style!`, and concatenates
the result.

```starlark
load("//bazel:scss_rules.bzl", "scss_rule")

scss_rule(
    name = "app_scss",
    output = "target/css/app.scss",
)
```

Then stage the output into a server crate:

```starlark
rust_rules(
    name = "server-lib",
    assets = [{
        "targets": [":app_scss"],
        "prefix": "target/css",
    }],
    crate_features = ["server"],
)
```

In Rust, import a nearby stylesheet and use the generated constants:

```rust
terrazzo_css::import_style!(style, "panel.scss");

#[html]
fn panel() -> XElement {
    div(class = style::PANEL, "Hello")
}
```

## Generated files

Use `generate_file` for generated artifacts that should be checked into the
repository. It creates an executable target tagged `auto-generated`, so the
merge-validation pipeline can refresh all generated files with:

```sh
bazel query 'attr("tags", "auto-generated", //...)' | xargs -r -n1 bazel run
```

Example:

```starlark
load("//bazel:generated_file.bzl", "generate_file")

generate_file(
    name = "pdfjs_pdf_min_mjs_update",
    src = "@pdfjs_dist//:build/pdf.min.mjs",
    dest = "assets/pdfjs/pdf.min.mjs",
)
```

When run, the target copies `src` into the package-relative `dest` under the
workspace. Set `ignore_whitespace = True` for generated files where formatting
noise should not cause a rewrite.

## Playwright rules

`playwright_setup` prepares Node and Playwright dependencies from the workspace
`package.json` and `package-lock.json`. The public instance is
`//bazel:playwright_setup`.

Use `playwright_test` for one browser test or `playwright_matrix_test` for a
debug/release matrix:

```starlark
load("//bazel:playwright_rules.bzl", "playwright_matrix_test")

playwright_matrix_test(
    overrides = {
        "text-editor-integration-test-debug": {
            "target_server": ":text-editor-server-debug",
        },
        "text-editor-integration-test-release": {
            "target_server": ":text-editor-server",
            "tags": ["opt_mode"],
        },
    },
    server = "//terminal/integration:exec",
    test = "tests/integration-test-text-editor.spec.mjs",
    extra_data = ["tests/PlantUML.pdf"],
)
```

The wrapper starts the requested server, points Playwright at it, and supplies
the target server's mirrored manifest and data files. Release-style tests are
usually tagged `opt_mode` so CI can run them with `-c opt`.

## Useful commands

```sh
bazel build //...
bazel test --test_output=errors --verbose_failures //...
bazel test --test_output=errors --verbose_failures -c opt \
  --flaky_test_attempts=3 $(bazel query 'attr("tags", "opt_mode", //...)')
bazel run //bazel:buildifier_check
bazel run //bazel:buildifier
```

If generated files may be stale:

```sh
bazel query 'attr("tags", "auto-generated", //...)' | xargs -r -n1 bazel run
```
