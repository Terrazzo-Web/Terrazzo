# Text editor terminal toggle

## Status

Proposed for review. Implementation has not started.

## Goal

Add a terminal toggle immediately before `manager.refresh_editor()` in
`terminal/src/text_editor/ui.rs`. Enabling it opens an embedded terminal on the
editor's remote, initialized in the editor's base folder. Loading an editor whose
tile already owns a terminal restores that terminal and shows the toggle as on.

## Proposed behavior

- Render the terminal below the editor with a resize bar, reusing the existing
  terminal UI and tile-owned terminal state.
- On load, list existing terminals and find terminals with the editor's exact tile
  ID and remote. Restore matching terminals without creating a shell or sending
  another `cd` command. Leave the toggle off when none exist.
- Derive the toggle from actual terminal existence; do not persist a separate
  enabled flag that can disagree with the process registry.
- On enable, create a terminal explicitly owned by the editor tile, using the
  current remote and resolved base folder after editor paths have been restored.
- On disable, close the matching terminal processes and remove the panel. If
  historical state contains multiple matching terminals, restore them using the
  existing tab UI and close all of them when disabling.
- Show pending state and suppress repeated clicks during creation or closure.
  Report failures and reconcile state instead of leaving a misleading toggle.
- Reconcile after shell exit, terminal closure, or ownership changes.
- Preserve the shell when the editor UI unmounts, so reopening can restore it.

## Ownership and orphan adoption

Currently `terminal/src/terminal/ui.rs` builds its ownership set from only
`App::Terminal` tiles. The first Terminal tile consequently displays terminals
whose owner is a text editor tile as though they were orphaned. This is fallback
filtering rather than a random reassignment of the stored tile ID.

- Include text editor tile IDs among valid terminal owners when deciding which
  terminals are orphaned.
- Retain first-Terminal-tile adoption for genuinely orphaned terminals, including
  terminals whose former tile was removed or switched to an unrelated app.
- Give embedded terminal discovery an exact-owner mode: it must never adopt
  orphaned terminals.
- Reuse terminal rendering without making an embedded terminal's selected tab
  change the editor's remote. The normal terminal UI currently updates its tile's
  remote from the selected terminal.
- Refresh both normal and embedded terminal views when ownership changes.

## Configuration and shell initialization

Add a server config option beside `terminal-shell`:

```yaml
server:
  terminal-cd: "cd -- {path}"
```

- Accept `terminal_cd` as an alias, following `terminal-shell` conventions.
- Default to `cd -- {path}` when omitted. Define `{path}` as an already
  shell-quoted absolute path; users should not add their own quotes around it.
- Validate the placeholder syntax and document the shell compatibility contract.
  The default and escaping target the existing POSIX shell setup; custom commands
  must be compatible with the configured shell.
- Wire the field through `backend/config/server.rs`, config merging and
  serialization in `merge.rs`, and dynamic updates in `into_dyn.rs`.
- Resolve the template from the config on the remote that owns the PTY, just as
  shell creation uses that remote's `terminal-shell`.
- Resolve relative base folders using the same rules as editor filesystem access.
- Carry initialization through the terminal creation/stream service to the PTY
  owner. Queue the expanded command once when a new shell starts, before accepting
  user input; do not race a browser write against lazy PTY creation.
- Do not initialize again on reconnect, snapshot restoration, or existing-process
  attachment. Keep ordinary terminals' startup behavior unchanged.
- Config updates affect newly created editor terminals.

## Implementation sequence

1. Add the config field, default, validation, merge/live-update support, and docs.
2. Add an optional editor startup context to the creation path, following the
   existing ServerFn and RemoteFn routing through `terminal/api.rs`,
   `terminal/client.rs`, and `terminal/service/`. Inspect the stream creation path
   to keep initialization bound to actual process creation.
3. Extract shared terminal state/rendering hooks from `terminal/ui.rs` as needed.
   Add exact-owner discovery and correct normal-terminal orphan filtering.
4. Add editor terminal state/lifecycle integration in `text_editor/manager.rs` and
   a focused editor terminal module. Ignore stale asynchronous results after a
   remote change or unmount, and prevent duplicate local creation requests.
5. Insert the toggle before refresh in `text_editor/ui.rs`; add active/pending
   styling, accessible labels, the terminal panel, and its resize control.
6. Add focused regression coverage and run relevant validation.

## Validation

- Ownership: a normal Terminal tile cannot adopt an editor-owned terminal; truly
  orphaned terminals still appear in the first Terminal tile.
- Restoration: existing exact tile/remote matches enable the toggle without
  creating another PTY or repeating initialization; unrelated terminals do not.
- Lifecycle: enable, disable, rapid repeated clicks, shell exit, failed creation,
  failed closure, editor reload, and remote changes keep the UI consistent.
- Initialization: verify the remote shell's working directory, including paths
  containing spaces, quotes, and shell metacharacters. Reconnect must preserve a
  manually changed working directory.
- Config: cover omission/default, custom template, alias, invalid placeholders,
  round-trip serialization, merge behavior, and live updates for new terminals.
- Browser: verify placement before refresh, active/pending state, resizing, and
  coexistence with a normal Terminal tile.
- Run focused Rust and browser integration targets for the changed paths, then
  relevant client/server builds and required checks from `terminal/AGENTS.md`.

## Review assumptions

- Disabling terminates the associated shell rather than merely hiding it.
- Changing the editor's base folder does not send commands into an existing
  shell; the current base folder is used on the next creation.
- The first version reuses the existing terminal tab controls in the embedded
  panel and treats all terminals matching the tile and remote as its terminals.
- No new CLI flag is required for `terminal-cd`; YAML configuration is sufficient.
- Terminal restoration remains limited to the existing server process registry;
  this feature does not preserve shells across server restarts.
