# Tabbed Tiles

## Summary
Add a third tile layout direction, `Direction::Tabbed`, so a tile array can show one child at a
time behind a tab strip. The tab strip should visually follow the existing terminal tabs: compact
titles across the top, selected-state styling, drag/drop reordering where practical, and the active
child filling the remaining tile area.

This is a layout feature for tiles, not a replacement for terminal tabs. A tabbed tile can contain
any app tile, including a terminal app whose own terminal tabs remain nested inside that selected
tile.

## Goals
- Extend the tile tree model with `Direction::Tabbed`.
- Render tabbed tile arrays with the shared `terrazzo::widgets::tabs` widget, matching the
  terminal tab interaction and style vocabulary.
- Add a menu action that converts the current tile into a tabbed group and then exposes a `+`
  affordance, like terminal tabs, to add more tile tabs.
- Preserve existing horizontal/vertical resize behavior.
- Persist tabbed layout through the existing tile tree server state.

## Non-Goals
- Do not merge terminal tabs and tile tabs into one concept.
- Do not add detachable/browser-window tabs.
- Do not require every app to provide a custom tile title in the first implementation.
- Do not add resize bars inside a tabbed group, because only one child is visible at a time.

## Data Model
- Add `Tabbed` to `terminal/src/tiles/api.rs`:
  - Use a compact serde rename when diagnostics are disabled, likely `#[serde(rename = "T")]`.
  - Keep `Clone`, `Copy`, `Debug`, `PartialEq`, and `Eq`.
- Update all exhaustive matches on `Direction`:
  - `terminal/src/tiles/ui.rs`
  - `terminal/src/frontend/mousemove.rs`
  - any tests or helper code found by `rg "Direction::" terminal/src`
- Keep the current `Tiles::Array { id, direction, nodes }` shape. No new top-level enum variant is
  needed.

## Server Mutations
- Update `terminal/src/tiles/api/add.rs` so adding a tabbed split works predictably:
  - If a tile is split with `Direction::Tabbed`, wrap the target tile and the new tile in a tabbed
    array.
  - Preserve the existing flattening behavior for horizontal-in-horizontal and vertical-in-vertical
    arrays.
  - Allow tabbed-in-tabbed flattening too, so adding a tab to an existing tabbed group
    appends/inserts into the same group instead of nesting tab strips.
- Add a dedicated conversion mutation for the menu action:
  - `tabify(id: TileId) -> Tiles`
  - If `id` is a leaf tile, replace it with
    `Tiles::Array { direction: Direction::Tabbed, nodes: vec![tile] }`.
  - If `id` is already a tabbed array or belongs to one, leave the tree unchanged and let the UI
    show the existing `+` button.
- Add a tab creation mutation for the `+` button:
  - `add_tab(array_id: TileId, after_child: Option<TileId>) -> Tiles`
  - Insert a default tile into the tabbed array and select it on the client.
- Add tests in `terminal/src/tiles/api/tests.rs`:
  - creating a tabbed group from one tile
  - inserting before/after in an existing tabbed group
  - converting a single tile into a one-tab tabbed group
  - adding a new tab through the tabbed-array `+` flow
  - ensuring horizontal/vertical flattening still behaves as today

Open decision: when a tabbed tile is inserted into a horizontal or vertical parent, it should behave
like any other child and occupy one flex slot. This keeps the tree semantics simple.

## Client State
- Extend `terminal/src/tiles/signals.rs` with a selected-tab signal for tabbed arrays.
  - Reuse the existing `TileSignals` visitor preservation pattern so selection survives server
    refreshes when the array id is stable.
- Persist selected tile in v1 through `tiles-state`, keyed by the tabbed array `TileId`, so reloads
  and tree refreshes keep the active tab.
- Add a small `TabbedTileState` struct:
  - `array_id: TileId`
  - `selected_tile: XSignal<TileId>`
  - title signals for each direct child tile
- When the `+` button creates a tab, update `selected_tile` to the new tile id after the server
  returns the updated tree.

## Rendering
- Split `show_tiles_rec` in `terminal/src/tiles/ui.rs` into three array render paths:
  - horizontal flex array
  - vertical flex array
  - tabbed array
- Keep the current resize-manager flow only for horizontal and vertical arrays.
- For `Direction::Tabbed`, render with `terrazzo::widgets::tabs::tabs`:
  - Create a `TileTabs` descriptor over the child nodes.
  - Create a `TileTab` descriptor for each child.
  - Implement `TabsState` for a small state object that owns the selected child signal and can
    reorder tabs.
  - `TabDescriptor::item` should call `show_tiles_rec` for the child with a single-child sizing
    context.
- Style the tabbed tile using the same class structure as terminal tabs where possible:
  - Either share terminal tab classes if they are appropriate and available without the `terminal`
    feature.
  - Or add tile-specific classes in `terminal/src/tiles/ui.scss` that mirror the terminal tab
    proportions and selected state.

Important layout detail: the tab strip needs fixed height and the selected child area must use
`min-height: 0`, `min-width: 0`, and flex growth so terminal/text-editor content can resize
correctly.

## Titles
- Add a tile title field in v1.
  - The title is only displayed when the tile is inside a tabbed array.
  - Default title is the app name; use `Debug` formatting if `App` does not implement `Display`.
  - Users can edit the tile-tab title with the same editable-title behavior used by terminal tabs.
  - Store title overrides with the tile model so they persist with the tabbed layout.
- Future enhancement: terminal tiles could surface the selected terminal tab title as the tile-tab
  title when no tile title override is set.

## Menu and Icons
- Add a third layout action in `terminal/src/frontend/menu.rs`:
  - existing horizontal split
  - existing vertical split
  - new tabbed conversion
- Add `Tile::tabify()` in `terminal/src/tiles/signals.rs`.
- Use `terminal/assets/icons/window-stack.svg` for the tabbed action:
  - Declare the icon in `terminal/src/assets/icons.rs`.
  - Install it server-side in `terminal/src/assets/install.rs`.
- Add non-prod test class, for example `split-tabbed`, matching the existing `split-horizontal` and
  `split-vertical` hooks.

## Reordering
- Use `terrazzo::widgets::tabs` drag/drop hooks for tab reordering.
- Add a server mutation for reordering children within an array:
  - `move_child(array_id: TileId, after_child: Option<TileId>, moved_child: TileId) -> Tiles`
  - Validate that both child ids belong directly to the same tabbed array.
  - Keep this scoped to `Direction::Tabbed` for v1.
- Include reordering in v1 because users will expect tile tabs to behave like terminal tabs.

## Styling
- Add tab styling in a new stylesheet, `terminal/src/tiles/tabs.scss`, imported by the tabbed tile
  rendering module.
- Include `tabbed-tile`, `tile-tabs`, `tile-tab-titles`, `tile-tab-title`, `tile-tab-items`,
  `tile-tab-item`, and `selected` classes.
- Match terminal tab visual rhythm:
  - top title strip
  - selected tab visibly connected to content
  - close enough spacing that nested terminal tabs do not look like a different product
- Avoid nested card styling. The tabbed tile is a layout container, not a card.

## Tests
- Rust unit tests:
  - tile tree insertion and flattening for `Direction::Tabbed`
  - tabify mutation tests
  - add-tab mutation tests
  - reorder server mutation tests
  - tile title persistence/defaulting tests
- Client/build validation:
  - `cargo build --bins --features=server,server-all,max_level_debug,debug,diagnostics`
  - `cargo build --bins --features=server,server-all,max_level_info --release`
  - `RUSTFLAGS="-A unused-crate-dependencies" cargo test --workspace --all-features`
- Bazel validation:
  - `bazel build //terminal/...`
  - targeted integration test if an existing tile/menu Playwright spec can be extended

## Suggested Implementation Order
1. Add `Direction::Tabbed` and update exhaustive matches with placeholder client behavior.
2. Add server insertion tests and implement tabbed flattening in `add.rs`.
3. Add the menu action and icon wiring.
4. Introduce client selected-tab state for tabbed arrays.
5. Render tabbed arrays through `terrazzo::widgets::tabs`.
6. Add tile tab styling and check nested terminal tabs manually.
7. Add tab reordering mutation and tests.
8. Run focused builds/tests, then broaden validation.

## Risks
- Nested tab strips can become visually noisy when a tabbed tile contains a terminal app. Matching
  spacing and selected-state styling matters.
- Terminal and editor views are sensitive to parent sizing. The selected item container must
  propagate resize events and keep `min-height: 0`.
- Existing tile array flattening is useful for split layouts but can create surprising tab nesting
  if `Tabbed` is not handled deliberately.
- Persisted tile trees from older versions will load fine, but newer trees containing `Tabbed` will
  not deserialize in older binaries.
