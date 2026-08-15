# Milkdown Markdown editor

The goal is to render `.md` text files with a two-pane Markdown editor modeled on
the Milkdown playground:

- the left pane is an editable Milkdown Crepe WYSIWYG view;
- the right pane is the Markdown source in the existing CodeMirror editor;
- edits in either pane synchronize the other pane and flow through the existing
  debounced `store_file` path so the Markdown is eventually written to disk;
- HTML preview, PDF viewing, and CodeMirror for other text files keep their current
  behavior.

The Milkdown playground implements this as two separate editors with guarded,
bidirectional synchronization. Milkdown itself does not provide the split view.
Terrazzo should follow that architecture rather than treating the source pane as a
Milkdown feature.

## Current editor flow

`terminal/src/text_editor/ui/editor.rs` currently chooses between three editor
bodies after the host element renders:

1. HTML text with preview enabled renders an iframe and has no mutable
   `EditorBody`.
2. Other text renders `CodeMirrorJs`.
3. PDF data renders `PdfJs`.

For mutable text, `CodeMirrorJs` emits the complete document through
`make_on_change`. Rust increments the pending-write counter and sends the content
through `fsio::client::store_file`, which coalesces and debounces writes per remote
and path. File notifications call `EditorBody::set_content` only when no local
write is pending, avoiding a common notification/write race. This save and reload
pipeline should remain the single persistence path for Markdown.

## Scope and behavior decisions

- Define a hardcoded fixed-size extension array, initially
  `static MARKDOWN_EXTENSIONS: [&str; 1] = ["md"];`, and route files through a
  shared `is_markdown` helper. This keeps the initial scope to `.md` while making
  future extension additions a one-line change.
- Both panes are editable. "Preview" means the WYSIWYG representation on the
  left, as in the playground, not a read-only rendered document.
- Keep the right pane on Terrazzo's current CodeMirror configuration, including
  Markdown syntax support, search, cursor persistence, and the input overlay.
- Keep the git-diff toggle for Markdown. When enabled, the left Crepe pane remains
  the editable WYSIWYG view and the right source pane uses the existing CodeMirror
  `MergeView`, with the original document read-only and the working document
  editable. Edits from the working side continue to synchronize Crepe and save to
  disk.
- Keep the HTML preview toggle restricted to `.html`; Markdown always selects its
  dedicated split editor.
- Store serialized Markdown, never rendered HTML or ProseMirror JSON.
- Use Milkdown Crepe's bundled editor experience rather than assembling a custom
  Milkdown kit in the first iteration. Crepe matches the requested playground UI
  and exposes Markdown change and replacement APIs.

## Task 1: Add and bundle Milkdown

Update `terminal/assets/jsdeps/package.json` and its lock file:

- add `@milkdown/crepe` and `@milkdown/kit` as direct dependencies, pinned by the
  lock file to the same compatible Milkdown release;
- use `@milkdown/kit` for the supported programmatic Markdown replacement utility
  needed when CodeMirror or the filesystem updates the WYSIWYG pane;
- add the Webpack CSS loader dependencies/configuration needed to bundle Crepe's
  common styles and one chosen theme. Prefer the dark theme that best matches the
  existing terminal UI, then override its layout colors locally where necessary.

Update `terminal/assets/jsdeps/src/index.js` to import and export through the
existing `window.JsDeps` bundle:

- `Crepe` from `@milkdown/crepe`;
- the Milkdown utility used to replace all Markdown without reconstructing the
  editor;
- `@milkdown/crepe/theme/common/style.css` and the selected Crepe theme CSS.

Regenerate and check in:

- `terminal/assets/jsdeps/package-lock.json` via `npm install`;
- `terminal/assets/jsdeps/dist/jsdeps.js` and any emitted CSS asset via
  `npm run build`;
- `terminal/terminal_assets.bzl` if Webpack emits a separate stylesheet. Install
  that stylesheet in `terminal/src/assets/install.rs` and link it from
  `terminal/assets/index.html`. If styles are injected into `jsdeps.js`, no new
  static asset is needed.

Do not import Milkdown directly from the wasm-bindgen snippet. Keeping third-party
packages in `JsDeps` preserves the repository's current JavaScript dependency and
asset pipeline.

Validation for this task:

```sh
cd terminal/assets/jsdeps
npm install
npm run build
```

Then build a text-editor server to prove that the generated browser assets are
packaged:

```sh
bazel build //terminal:text-editor-server-debug
```

## Task 2: Add a Markdown editor bridge

Create:

- `terminal/src/text_editor/ui/milkdown.rs`, a Rust wasm-bindgen wrapper analogous
  to `code_mirror.rs`;
- `terminal/src/text_editor/ui/milkdown.js`, which owns both the Crepe instance and
  a `CodeMirrorJsImpl` source instance;
- `terminal/src/text_editor/ui/milkdown.scss` for the split-pane layout and local
  Crepe theme integration.

Register the modules from `terminal/src/text_editor/ui.rs`.

### Rust wrapper

Define `MilkdownJs` with the same useful lifetime guarantees as `CodeMirrorJs`:

- retain the Rust `onchange` and `oncursor` closures for as long as JavaScript may
  call them;
- call `destroy` from `Drop`;
- expose `set_content`, `insert_text`, `focus`, and (if the embedded CodeMirror
  retains it) `cargo_check` so it can implement the existing `EditorBody` trait;
- expose or log asynchronous Crepe creation failures instead of leaving a blank
  pane with an unhandled promise rejection.

The constructor should receive the optional original document, current content,
callbacks, cursor position, base path, and full path. The source CodeMirror must
receive the same `original` value currently selected by `show_editor_diff`, along
with the same path data, so the existing editable/merge behavior, language
selection, and future diagnostics continue to work normally.

### JavaScript owner and lifecycle

`MilkdownJsImpl` should create two stable child hosts inside the Rust-provided
editor element:

- `.milkdown-wysiwyg-pane` on the left;
- `.milkdown-source-pane` on the right.

Create Crepe with the loaded Markdown as `defaultValue`, and construct
`CodeMirrorJsImpl` in the source host with the same content and optional original
document. In diff mode, synchronization must always read and update the editable
`b` editor selected by `CodeMirrorJsImpl`; the read-only original must never emit a
save. The outer wrapper, not either child editor, owns synchronization and the
Rust save callback.

Crepe creation is asynchronous. Track `creating`, `ready`, and `destroyed` state so
that:

- a `set_content` received before creation finishes becomes the value applied once
  ready;
- destruction before creation finishes still destroys the eventual Crepe instance
  and never installs callbacks into a dead editor;
- `destroy` is idempotent and destroys both Crepe and CodeMirror;
- no change callback runs after destruction.

### Bidirectional synchronization

Use full Markdown strings as the synchronization boundary:

1. Crepe's `markdownUpdated` listener receives serialized Markdown.
2. If the change is user-originated, update CodeMirror and invoke the Rust
   `onchange(markdown)` callback.
3. CodeMirror's document listener receives source Markdown.
4. If the change is user-originated, replace Crepe's full Markdown document and
   invoke the same Rust `onchange(markdown)` callback.
5. `set_content` from Rust updates both panes without invoking `onchange`.

Use explicit per-direction suppression (or a small update-origin state machine),
not only string equality. Milkdown serialization may normalize Markdown, so
equality alone is insufficient to prevent update loops. After a peer update,
compare the final serialized Markdown and settle both panes on one canonical
string without scheduling a second disk save.

Throttle only expensive pane-to-pane Milkdown parsing if profiling shows it is
needed. Do not add a second persistence debounce: `fsio::client::store_file`
already coalesces changes and is the authority for save timing and synchronized
state.

`insert_text` should insert into the CodeMirror source selection and let the normal
source-to-Crepe synchronization path handle the WYSIWYG update. Track the most
recently focused pane so `focus()` can restore focus there, defaulting to Crepe on
initial load. Forward cursor changes only from the CodeMirror pane because the
persisted cursor format uses source-document offsets and cannot safely be inferred
from a ProseMirror selection without a mapping layer.

## Task 3: Select Milkdown for configured Markdown extensions

Update `terminal/src/text_editor/ui/editor.rs`:

- add `static MARKDOWN_EXTENSIONS: [&str; 1] = ["md"];` and a shared
  `is_markdown(path: &Path) -> bool` helper, then use that helper everywhere the
  UI needs to classify a Markdown file;
- render a `milkdown-editor` test class and the imported Milkdown layout class;
- keep the input overlay enabled because the Markdown body remains editable;
- instantiate `MilkdownJs` for Markdown text, `CodeMirrorJs` for other mutable
  text, the iframe for HTML preview, and `PdfJs` for PDFs;
- pass the optional original content to `MilkdownJs` under the same
  `show_editor_diff` condition used for ordinary CodeMirror files, so its source
  pane switches between a plain editor and the existing merge view;
- reuse the exact existing `make_on_change`, `make_on_cursor_position_change`,
  `writing`, file watcher, and `notify_edit` plumbing;
- ensure filesystem reloads call `MilkdownJs::set_content`, which updates both
  panes while suppressing save callbacks.

Keep `toggle_editor_diff` visible for modified Markdown files, using the same
`original != content` rule as other text. Update `is_focusable`: it currently
assumes preview state is controlled only by `show_html_preview`; the focusable
class should instead reflect whether the selected editor body is interactive,
including Markdown regardless of the HTML preview signal.

No server/fsio changes should be necessary. If implementation appears to require a
new save endpoint, stop and reassess the bridge first: Milkdown produces a Markdown
string compatible with the existing text-file endpoint.

## Task 4: Style the two-pane editor

In `milkdown.scss` and the existing text-editor layout:

- use a horizontal 50/50 flex split with `min-width: 0` on both panes;
- give each pane independent vertical scrolling and full editor height;
- add a visible divider while using existing terminal color variables;
- constrain Crepe/ProseMirror descendants so long content does not expand the
  outer tile or force the source editor off screen;
- keep CodeMirror's current dark background and make the Crepe theme visually
  compatible with it;
- in diff mode, keep both CodeMirror merge editors contained within the right half
  with their existing horizontal overflow behavior; do not allow the merge view
  to resize or cover the Crepe pane;
- add a responsive narrow-width rule that stacks the WYSIWYG pane above the source
  pane if two usable columns cannot fit;
- verify tooltips, slash menus, link popovers, and code-block editors are not
  clipped by the parent editor's current `overflow: hidden` rule;
- add non-obfuscated classes under `#[cfg(not(feature = "client-prod"))]` for the
  outer Markdown editor and both panes so Playwright selectors remain stable.

The first version does not need a draggable pane divider. The equal split and
responsive fallback are enough to establish the feature without adding another
persisted UI state.

## Task 5: Add an integration test

Add `terminal/tests/integration-test-text-editor-markdown.spec.mjs` and matching
debug/release `playwright_matrix_test` targets in `terminal/BUILD.bazel`. Include
`tests/text-editor-helpers.mjs` as test data, and add helpers for the Milkdown host,
ProseMirror content, and Markdown source pane.

The primary test should:

1. Create a temporary `.md` file containing headings, emphasis, a link, a list,
   and a fenced code block.
2. Open it through the existing base-path and folder helpers.
3. Assert that the two panes are visible, CodeMirror is not being used as the sole
   editor, and representative Markdown is rendered structurally in the Crepe
   pane.
4. Edit the WYSIWYG pane using normal browser interaction.
5. Assert that the right CodeMirror pane receives the corresponding Markdown.
6. Poll the file on disk and assert that the Markdown change is persisted through
   the existing debounced save path.
7. Edit the Markdown source pane.
8. Assert that the left Crepe pane updates and that the new source is eventually
   persisted to disk.
9. Modify the file directly on disk after local writes have settled and assert
   that both panes reload to the external content without causing a save loop.
10. Use a Markdown fixture with committed original content, edit it, reopen it,
    enable the diff toggle, and assert that the right pane contains the original
    and editable working CodeMirror views while the left Crepe pane remains
    visible. Edit the working view and verify Crepe and the disk file update.
11. Open a `.txt` file and assert it still uses the single CodeMirror editor; open
    an `.html` file and assert its current preview/source toggle still works.

Avoid asserting Milkdown's complete serialized Markdown byte-for-byte after rich
editing because its serializer may normalize whitespace or markers. Assert the
specific semantic change in both the source pane and disk file. For direct source
edits, where Terrazzo controls the exact string, an exact disk assertion is
appropriate.

Run both configurations:

```sh
bazel test --test_output=errors //terminal:text-editor-markdown-integration-test-debug
bazel test --test_output=errors //terminal:text-editor-markdown-integration-test-release
```

Then run the existing text-editor basic and viewer suites to catch editor routing
and HTML regressions:

```sh
bazel test --test_output=errors \
  //terminal:text-editor-basic-integration-test-debug \
  //terminal:text-editor-viewer-integration-test-debug
```

## Completion criteria

- Opening `.md` displays editable Crepe and Markdown source panes side by side.
- Markdown detection is centralized in a fixed-size extension array that can be
  extended without changing editor routing call sites.
- Editing either pane updates the other without oscillation, duplicate saves, or
  cursor jumps in the actively edited pane.
- The Markdown diff toggle keeps Crepe visible and renders the original/working
  merge view within the source pane; only working-side changes are synchronized
  and saved.
- Markdown changes reach the existing synchronization indicator and are written
  to the selected local or remote file.
- An external file update refreshes both panes without being echoed back as a
  local edit.
- Switching files or destroying a tile cleans up both JavaScript editors even if
  Crepe is still loading.
- `.txt`, `.html`, and PDF behavior remains unchanged.
- New Playwright tests pass in debug and release configurations.

## References

- Milkdown getting started and Crepe installation:
  https://milkdown.dev/docs/guide/getting-started
- Crepe events, lifecycle, and `getMarkdown`:
  https://milkdown.dev/docs/guide/using-crepe
- Milkdown programmatic content replacement:
  https://milkdown.dev/docs/guide/faq
- Official playground split-view coordinator:
  https://github.com/Milkdown/website/blob/main/src/components/playground/index.tsx
- Official playground Crepe synchronization implementation:
  https://github.com/Milkdown/website/blob/main/src/components/playground/Crepe.tsx
