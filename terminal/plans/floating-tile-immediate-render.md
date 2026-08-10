# Render a floated terminal immediately

## Objective

Fix the live tile-tree transition where clicking the right tile's
`window-stack.svg` action persists the terminal as a floating tile, but the browser does not render
the floating window until the page is refreshed.

Expected behavior: the terminal moves into an approximately 800 x 600 floating window immediately
after the click. A reload must not be required.

## Commit 1: Document and verify the reproduction setup

Suggested commit message: `Document the immediate floating-tile reproduction`

Record this deterministic setup and verify it manually against a freshly restarted terminal server:

1. Start the integration server from the repository root and leave it running:

   ```sh
   terminal/tests/run-server.sh
   ```

2. Open the terminal application and authenticate when required.
3. Hover the current tile's menu trigger, identified by
   `/static/icons/signpost-split.svg` or the non-production `.app-menu-trigger` test hook.
4. Click `/static/icons/arrows-expand-vertical.svg`, exposed to tests as
   `.split-horizontal`, and wait until two `.app-tile` elements are visible.
5. Hover the right tile's `.app-menu-trigger` and select the Terminal app by clicking its menu
   item containing
   `/static/icons/terminal-dash.svg`.
6. In the right tile, click its `.add-tab-icon img` (`+`) and wait for one terminal tab and its
   `.xterm` view to become visible.
7. Hover the right tile's menu trigger again and click
   `/static/icons/window-stack.svg`, exposed as `.float-tile`.
8. Observe the bug without refreshing:
   - the split layout is replaced by the tabbed host/default tile;
   - the terminal is not visible as `.floating-tile`;
   - after a refresh, the same persisted terminal appears in a floating window at the default
     position and size.

Keep this commit documentation-only. Do not include the test or production fix yet.

## Commit 2: Add a failing Playwright integration test

Suggested commit message: `Test floating a populated terminal tile`

Add a focused Playwright scenario under `terminal/tests` and wire it into `terminal/BUILD.bazel`.
Prefer a dedicated tile-layout spec/target so it gets a fresh integration server and cannot leak its
persisted tile tree into the existing terminal-tab tests.

The test will reproduce the UI sequence from commit 1 using stable non-production hooks rather
than coordinates:

- locate the first tile's `.app-menu-trigger`, hover it, and click `.split-horizontal`;
- assert that exactly two regular `.app-tile` elements are visible;
- scope all subsequent app selection and terminal creation actions to the second/right tile;
- select the Terminal menu item, click the scoped `.add-tab-icon img`, and wait for `Terminal 1`
  and a visible `.xterm`;
- hover the right tile's menu and click its `.float-tile` control;
- without calling `reload()`, assert that one visible `.floating-tile` exists, contains the terminal
  tab and `.xterm`, and has the default 800 x 600 geometry;
- assert that the regular host/default tile remains behind the floating window.

Run the new debug Bazel target and confirm that the final immediate-render assertion fails on the
current implementation. Commit that regression test separately before changing production code.

## Commit 3: Fix live reconciliation of the floated tile

Suggested commit message: `Render floated tiles immediately after tree updates`

Trace the click from `Tile::float` through `RootTree::update` and compare three states:

1. the server DTO returned by `tiles::api::float`;
2. the transformed client tree from `Tiles::update`;
3. the DOM emitted by `show_tabbed_tiles` and `show_floating_tiles` during that same update.

The fact that refresh renders the correct floating window means the persisted server tree is
already sufficient. Fix the earliest client/rendering layer that drops the newly moved keyed tile
during the live transition. The likely boundary is reconciliation while the same tile identity
moves from a regular `nodes` branch into `floating_nodes`, rather than the server-side float model.

Implementation requirements:

- preserve the existing `TilePtr`/signal reuse behavior and terminal session identity;
- render the new `.floating-tile` during the response-driven `RootTree::update`;
- avoid cloning the terminal into both regular and floating branches;
- keep reload persistence, floating position/size, z-index raising, and the inverse
  floating-to-regular action working;
- add a focused Rust regression test if the fix changes tree transformation, identity comparison,
  or another unit-testable helper; keep the Playwright test as the end-to-end guard.

Validation for this commit:

- run the new debug Playwright target and confirm it now passes without reload;
- run `cargo test -p terrazzo-terminal --all-features` or the narrow equivalent required by the
  touched modules;
- run the existing tile API tests, especially `tiles::api::float`;
- run `//terminal:terminal-integration-test-debug` to protect existing terminal-tab behavior;
- run the release/opt-mode version of the new test when the focused debug checks are green;
- repeat the new target with `--runs_per_test=10` to catch timing-sensitive regressions.

## Commit discipline

Each numbered section is one commit, in order. Stage only that section's files/hunks. Preserve and
exclude unrelated worktree changes, including the existing editor settings change.
