# SyncTeX parser Rust crate

The goal of this project is to create a Rust crate that exposes the public API from
[`synctex_parser.h`](https://github.com/jlaurens/synctex/blob/main/synctex_parser.h).

The crate should make the SyncTeX scanner usable from Rust code in this workspace, while keeping the
FFI boundary small, auditable, and compatible with Bazel and Cargo builds. The PDF viewer is the
expected future consumer, but this plan does not wire the crate into the PDF viewer yet.

## Scope

- Expose the `synctex_parser.h` API:
  - scanner lifecycle:
    - `synctex_scanner_new_with_output_file`
    - `synctex_scanner_free`
    - `synctex_scanner_parse`
  - forward and reverse queries:
    - `synctex_display_query`
    - `synctex_edit_query`
    - `synctex_scanner_next_result`
    - `synctex_scanner_reset_result`
  - scanner metadata:
    - `synctex_scanner_get_name`
    - `synctex_scanner_get_tag`
    - `synctex_scanner_input`
    - `synctex_scanner_input_with_tag`
    - `synctex_scanner_get_output`
    - `synctex_scanner_get_synctex`
    - `synctex_scanner_get_output_fmt`
    - `synctex_scanner_x_offset`
    - `synctex_scanner_y_offset`
    - `synctex_scanner_magnification`
    - `synctex_scanner_dump`
    - `synctex_scanner_display`
  - node source mapping:
    - `synctex_node_tag`
    - `synctex_node_line`
    - `synctex_node_mean_line`
    - `synctex_node_column`
    - `synctex_node_get_name`
    - `synctex_node_page`
  - node tree traversal:
    - `synctex_node_parent`
    - `synctex_node_parent_sheet`
    - `synctex_node_parent_form`
    - `synctex_node_child`
    - `synctex_node_last_child`
    - `synctex_node_sibling`
    - `synctex_node_last_sibling`
    - `synctex_node_arg_sibling`
    - `synctex_node_next`
    - `synctex_sheet`
    - `synctex_sheet_content`
    - `synctex_form`
    - `synctex_form_content`
  - visible and TeX geometry:
    - `synctex_node_visible_*`
    - `synctex_node_box_visible_*`
    - `synctex_node_h`
    - `synctex_node_v`
    - `synctex_node_width`
    - `synctex_node_height`
    - `synctex_node_depth`
    - `synctex_node_box_*`
  - debug helpers:
    - `synctex_node_log`
    - `synctex_node_display`
- Keep `synctex_parser_advanced.h` out of the first milestone unless a future PDF viewer
  integration later needs concurrent query iterators.
- Treat the upstream C structs as opaque. Rust code must not depend on their layout.

## Status Quo

- The workspace does not currently contain a SyncTeX crate or C FFI build pipeline.
- The root `Cargo.toml` uses a workspace with local crates and shared dependency declarations.
- Bazel uses `rules_rust` and generated crate repositories, so any new crate must be represented in
  both Cargo and Bazel.
- The terminal text editor can display PDFs, but this plan only creates the reusable SyncTeX crate.
  PDF viewer integration should happen in a later, separate plan.

## Crate Layout

- Add a low-level crate, for example `utils/synctex/sys`, with package name `terrazzo-synctex-sys`.
  - Own vendored C sources and generated/raw FFI declarations.
  - Expose unsafe bindings that mirror `synctex_parser.h`.
  - Keep names close to the C API for easy diffing against upstream.
- Add a safe wrapper crate, for example `utils/synctex/synctex`, with package name
  `terrazzo-synctex`.
  - Own Rust-safe `Scanner`, `Node`, query result, geometry, and error types.
  - Depend on `terrazzo-synctex-sys`.
  - Provide the API that future workspace consumers, such as the PDF viewer, should use.
- Add both crates to the root Cargo workspace.
- Add both crates to `workspace.dependencies` only if that matches the workspace's local-crate
  conventions. Do not add them as terminal dependencies in this plan.

## Upstream Source Strategy

- Vendor the upstream SyncTeX parser source files into the `*-sys` crate.
  - Include `synctex_parser.h`, `synctex_parser.c`, `synctex_version.h`, and any private headers or
    companion sources required to compile the parser.
  - Include upstream license/copyright text in the vendored directory.
  - Record the exact upstream commit SHA in a `README.md` or `UPSTREAM.md`.
- Prefer vendoring over linking to a system library for the first version.
  - This avoids platform package drift and makes Bazel/Cargo builds reproducible.
  - A future `system-synctex` feature can be added if downstream packaging needs it.
- Check whether gzip support is required by the upstream parser build.
  - SyncTeX commonly reads both `.synctex` and `.synctex.gz`.
  - If zlib is required, decide whether to vendor/minimize that dependency, use a crate-compatible C
    dependency, or make compressed input support a separately tested feature.

## FFI Binding Plan

- Start with a small hand-written FFI module rather than full bindgen output.
  - The public API in `synctex_parser.h` is modest and mostly opaque pointers, integers, floats, and
    C strings.
  - Hand-written declarations reduce generated-code churn and are easier to audit in this workspace.
- Mirror the C types exactly:
  - `synctex_status_t` maps to `libc::c_long`.
  - `synctex_scanner_p` and `synctex_node_p` map to opaque pointer newtypes or raw pointers to
    zero-sized opaque structs.
  - C strings use `*const libc::c_char`.
  - C `float` maps to `libc::c_float`.
- Keep all raw functions inside an `unsafe` `sys` module.
- Add a compile-time check for pointer-sized opaque handles if using wrapper newtypes.
- Include a tiny C smoke test or Rust FFI test that links the parser and calls a harmless function
  path, such as creating a scanner for a non-existent output and receiving a null scanner.

## Safe API Plan

- Define `Scanner` as the owning Rust handle.
  - `Scanner::new_with_output_file(output, build_directory, parse) -> Result<Option<Scanner>, Error>`
    or a clearer split such as `Scanner::open(...) -> Result<Scanner, Error>`.
  - Implement `Drop` by calling `synctex_scanner_free`.
  - Make `Scanner` `!Send` and `!Sync` unless upstream thread-safety is proven.
- Define `Node<'scanner>` as a borrowed handle tied to a scanner lifetime.
  - Query result nodes are owned by the scanner; Rust must not outlive the scanner.
  - Tree traversal methods return `Option<Node<'scanner>>`.
- Expose query methods:
  - `Scanner::display_query(input, line, column, page_hint) -> Result<QueryResults<'_>, Error>`
  - `Scanner::edit_query(page, h, v) -> Result<QueryResults<'_>, Error>`
  - `QueryResults` should iterate using `synctex_scanner_next_result`.
  - Reset support can be represented as `QueryResults::reset()` or `Scanner::reset_result()`.
- Convert returned C strings as borrowed `CStr` first.
  - Provide lossy/path-oriented helpers where terminal UI needs display strings.
  - Avoid assuming every SyncTeX path is valid UTF-8.
- Represent geometry with small structs:
  - `VisibleBox { h, v, width, height, depth }` in page coordinates.
  - `TexBox { h, v, width, height, depth }` in TeX small points.
  - `Point { h, v }` where useful.
- Represent negative `synctex_status_t` values as `Error::Status`.
  - Non-negative values are counts.
  - A null scanner after parse/open should become a structured error or `None`, not a panic.
- Keep debug dump/display functions available behind explicit methods, but avoid calling stdout
  debug helpers from production code paths.

## Bazel Integration

- Add `BUILD.bazel` files for both crates.
- Use the repo-local Rust wrapper for Rust compilation:
  - load `rust_rules` with `load("//bazel:rust_rules.bzl", "rust_rules")`
  - use `rust_rules(...)` for the `terrazzo-synctex-sys` and `terrazzo-synctex` Rust targets
  - use `rust_rules_matrix(...)` only if the crate needs feature/platform variants, matching the
    pattern used by terminal targets
  - do not call raw `rust_library`, `rust_binary`, or `rust_test` directly from the crate
    `BUILD.bazel` files unless the local wrapper cannot express a required native-linking detail
- For the `*-sys` crate:
  - Add a `cc_library` for vendored SyncTeX C sources.
  - Add a `rust_rules(...)` target whose `deps` include that `cc_library`.
  - Ensure C include paths expose the vendored header directory.
  - Add zlib or compressed-input dependencies if required.
- For the safe wrapper crate:
  - Add a `rust_rules(...)` target depending on `terrazzo-synctex-sys`.
  - Rely on the wrapper-generated test, rustfmt, clippy, and build-test targets unless custom
    fixture/data wiring requires extra targets.
- Update any generated crate-alias or feature-dependency flows if the workspace requires it for new
  local crates.
- Validate that both Cargo and Bazel can build the crates before any consumer is added.

## Cargo Integration

- Add `Cargo.toml` files for the new crates.
- Add `build.rs` to the `*-sys` crate if Cargo needs to compile vendored C sources with the `cc`
  crate.
- Add `cc` and any other build dependencies to root `workspace.dependencies` if the repo prefers
  workspace-managed versions.
- Regenerate `Cargo.lock` if new crates or dependencies are introduced.
- Keep the default features minimal:
  - default: parser support for plain `.synctex`
  - optional: gzip support if it introduces extra native dependencies

## Test Fixtures

- Add small SyncTeX fixtures under the safe crate.
  - Include a tiny `.tex`, `.pdf`, and `.synctex` pair if feasible.
  - Prefer checked-in fixtures over invoking TeX during normal tests.
- Include at least one source-to-PDF display query fixture.
- Include at least one PDF-to-source edit query fixture.
- Include tests for:
  - scanner open failure on missing files
  - scanner parse success on fixture
  - display query returns at least one node with expected page/line/name
  - edit query returns a plausible source line
  - result iteration ends cleanly
  - tree traversal does not produce dangling references
  - visible and TeX geometry methods return stable values for a known fixture

## Implementation Tasks

### Task 1: Vendor upstream and create `terrazzo-synctex-sys`

- Add the vendored SyncTeX parser sources and upstream metadata.
- Create the sys crate with raw FFI declarations.
- Compile the vendored C parser in Cargo and Bazel.
- Add a minimal link/smoke test.

Validate with:

    cargo test -p terrazzo-synctex-sys
    bazel test //utils/synctex/sys:all

Create a git commit for this task.

### Task 2: Create the safe `terrazzo-synctex` wrapper

- Add `Scanner`, `Node`, query result iterator, geometry structs, and error types.
- Implement scanner lifecycle and query wrappers.
- Implement node metadata, tree traversal, and geometry wrappers.
- Keep unsafe calls isolated and documented at the boundary.

Validate with:

    cargo test -p terrazzo-synctex
    bazel test //utils/synctex/synctex:all

Create a git commit for this task.

### Task 3: Add real SyncTeX fixtures and behavioral tests

- Add checked-in fixture files.
- Test display query, edit query, scanner metadata, result reset, and representative geometry.
- Confirm fixture paths work on macOS/Linux and under Bazel runfiles.
- Avoid tests that depend on locally installed TeX.

Validate with:

    cargo test -p terrazzo-synctex
    bazel test //utils/synctex/...

Create a git commit for this task.

### Task 4: Keep terminal integration out of scope

- Do not wire `terrazzo-synctex` into `terrazzo-terminal` yet.
- Do not add terminal features, PDF viewer behavior, or Playwright integration tests in this plan.
- Leave a short note in the crate README, if useful, that the intended future consumer is the PDF
  viewer.
- The only terminal-adjacent validation for this plan is that adding the new workspace crates does
  not break existing workspace/Bazel discovery.

Validate with:

    cargo test -p terrazzo-synctex-sys
    cargo test -p terrazzo-synctex
    bazel test //utils/synctex/...

Create a git commit for this task only if it makes documentation or workspace-registration changes
that were not already covered by Tasks 1-3.

## Future PDF Viewer Integration

This crate is expected to be used by the PDF viewer later, but that work belongs in a separate plan.
That future plan should cover:

- adding `terrazzo-synctex` as a terminal dependency
- deciding which terminal feature owns SyncTeX support
- using `Scanner::display_query` for source-to-PDF navigation
- using `Scanner::edit_query` for PDF-to-source navigation
- converting SyncTeX page-space geometry into the PDF viewer coordinate system
- adding PDF viewer integration tests

## Rust API Sketch

```rust
pub struct Scanner {
    raw: NonNull<sys::synctex_scanner_t>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Scanner {
    pub fn open(output: &Path, build_directory: Option<&Path>) -> Result<Self, Error>;
    pub fn parse(&mut self) -> Result<(), Error>;
    pub fn display_query(
        &mut self,
        input: &Path,
        line: i32,
        column: i32,
        page_hint: i32,
    ) -> Result<QueryResults<'_>, Error>;
    pub fn edit_query(&mut self, page: i32, h: f32, v: f32) -> Result<QueryResults<'_>, Error>;
}

pub struct QueryResults<'scanner> {
    scanner: &'scanner mut Scanner,
    remaining_hint: usize,
}

pub struct Node<'scanner> {
    raw: NonNull<sys::synctex_node_t>,
    _scanner: PhantomData<&'scanner Scanner>,
}
```

The final API does not need to match this sketch exactly, but it should preserve these ownership
rules: scanner owns parser state, nodes borrow from the scanner, and query iteration mutably borrows
the scanner while results are being consumed.

## Risks And Mitigations

- Upstream source layout may require more than `synctex_parser.c`.
  - Mitigate by vendoring the minimal compiling source set and documenting it in `UPSTREAM.md`.
- `.synctex.gz` support may introduce native zlib complexity.
  - Mitigate by deciding on gzip support explicitly in Task 1 and testing both compressed and
    uncompressed fixtures if enabled.
- SyncTeX paths are byte strings, not guaranteed UTF-8.
  - Mitigate by exposing `CStr`/byte-oriented accessors and only lossy-converting in UI layers.
- Query results are invalidated by the next query.
  - Mitigate with a `QueryResults<'_>` type that holds a mutable borrow of `Scanner`.
- Thread-safety is unknown.
  - Mitigate by keeping `Scanner` `!Send`/`!Sync` until upstream guarantees are understood.
- Bazel/Cargo native build flags may drift.
  - Mitigate by keeping source lists and compile defines mirrored in one documented place.

## Validation

- Run formatting:

    cargo +nightly fmt

- Run crate-level validation:

    cargo test -p terrazzo-synctex-sys
    cargo test -p terrazzo-synctex
    bazel test //utils/synctex/...

- Run broader workspace validation if touching shared Bazel/Cargo metadata:

    bazel build //...

## Assumptions

- The first version should expose `synctex_parser.h`, not the advanced iterator API.
- Vendoring upstream SyncTeX is acceptable because the header license permits reuse with copyright
  and permission notices preserved.
- A safe wrapper crate is worth creating immediately because direct use of raw C pointers would make
  query-result lifetimes and scanner ownership too easy to misuse.
- The PDF viewer SyncTeX UI work should happen later, after the crate has standalone tests and a
  stable Rust API.
