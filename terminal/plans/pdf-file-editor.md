# PDF file editor

The goal of this project is to create a PDF file editor.

## Plan review

Review this plan. Fix typos, rephrase for clarity if necessary.

Create a git commit with your changes.

## Testing infrastructure

The first step is to make sure we have appropriate testing infrastructure for the existing text editor.

### Task 1.1: Create build targets for text-editor

Notice how the server can be built with the `text-editor` feature enabled for both the client and server.
1. The features are declared in terminal/Cargo.toml
2. We use the Bazel rule `feature_deps` in terminal/BUILD.bazel to copy them over to terminal/terminal_features.bzl, so features are easy to use in Bazel rules.
3. We declare multiple `terminal_rules` with different sets of features, but we don't have build targets specifically for the `text-editor` feature set. It would be nice to avoid rebuilding the full feature set when testing only the text editor.

Create a git commit for this task.

Validate with `bazel build //terminal/...`

### Task 1.2: Create Playwright integration tests for text editor

1. Create an `integration-test-*.spec.mjs` test case like the other test cases in terminal/tests.
2. Create the matching `playwright_matrix_test` Bazel rule in terminal/BUILD.bazel
3. The test should leverage the text-editor specific target to run the server.
4. First, create a test that starts the server and does nothing else. This validates that the code builds and the test runs.

Validate with:

    bazel test //terminal:text-editor-integration-test-debug
    bazel test //terminal:text-editor-integration-test-release

(assuming the new test targets are called `text-editor-integration-test-debug` and `text-editor-integration-test-release`)

Create a git commit for this task.

### Task 1.3: Add a simple test that edits a file

#### Part 1: Description of the test we want to achieve

In `integration-test-text-editor.spec.mjs`, created in Task 1.2, add a test that edits a file.

1. First, the file must exist, so `integration-test-text-editor.spec.mjs` must create an empty temp file.
2. Open the file by navigating to the folder of the temp file in `base_path_selector`.
3. The list of files in the temp folder should show up in the `editor_body`; click on the name of the temp file to open it.
4. The CodeMirror editor shows up.
5. Write "Hello, world!" by typing it in the CodeMirror editor.
6. After a delay, the changes get saved.
7. Verify that the file contains "Hello, world!" by reading it directly from disk.

#### Part 2: Prerequisites

In order for Playwright to act on the dynamic HTML page, it needs to be able to locate nodes. However, node classes are obfuscated on purpose, so they are not usable for tests.

The solution is to add plaintext classes to nodes.

```rust
#[cfg(not(feature = "client-prod"))]
class = "app-menu-trigger",
```

The classes should be gated by `not(feature = "client-prod")` so they are not built into the production binary but are enabled in integration tests, including integration tests running with `-c opt`.

Create a git commit for this task.

#### Part 3: Implementation

Implement the test described in Part 1.

Validate with:

    bazel test //terminal:text-editor-integration-test-debug
    bazel test //terminal:text-editor-integration-test-release

Create a git commit for this task.

## PDF file editor

### Task 1: Understand existing

- The method `editor_container` displays the editor body, which is one of:
  - a `fsio::File::TextFile` displayed as CodeMirror editor
  - a `fsio::File::Folder` displayed using the `folder` view
  - an error
  - the new `fsio::File::PdfFile` case. This is not wired through yet; it only displays the length of the file in a CodeMirror editor. The CodeMirror editor should be removed and replaced with the PDF viewer.

- The method `notify_edit` reloads the file if it changed on disk
  - The `CodeMirrorJs` type needs to be abstracted behind a trait
  - Replace `CodeMirrorJs` with something like `Box<dyn EditorBody>`
  - Then the code for the PDF file case will be similar to the text file case. The difference is that PDF files may not be UTF-8, so the content is base64-encoded.

---

TODO: Edit the present `terminal/plans/pdf-file-editor.md` file and put your summary in this section between the two horizontal lines.

Include
- how the CodeMirror is configured
- how the file is watched for edits
- what happens to the UI when the file is reloaded: do we always rebuild and replace the UI, lose pending changes, scroll back to the top, etc.
- suggest 1 or 2 improvements

---

Create a git commit for this task.

### Task 2: Introduce PDF viewer

First, you need to:
- download and add PDF.js from Mozilla in terminal/assets
- install it in terminal/src/assets/install.rs
- load it from terminal/assets/index.html (or maybe load it on-demand first time PDF file is opened)

Validate with:

    bazel test //terminal:text-editor-integration-test-debug
    bazel test //terminal:text-editor-integration-test-release

Create a git commit for this task.

### Task 3: Implementation

Implement `EditorBody` trait for PDF.js

Validate with:

    bazel build //terminal:text-editor-integration-test-debug

(do not run the tests, just build)

Create a git commit for this task.

### Task 4: Test

See file terminal/tests/PlantUML.pdf: we're going to use this file to test the PDF viewer works end-to-end.

Add it to the Bazel test targets `//terminal:text-editor-integration-test-debug` and `//terminal:text-editor-integration-test-release` so it is available during tests.

Add a test method that starts the server and opens a Playwright browser, opens the text-editor app, navigates to the base path folder, selects PlantUML.pdf from the folder view.

Then validate that the PDF file shows. If it is too hard (because it's canvas), propose a methodology and add a Task 5. with suggestions.

Validate with:

    bazel test //terminal:text-editor-integration-test-debug
    bazel test //terminal:text-editor-integration-test-release

Create a git commit for this task.
