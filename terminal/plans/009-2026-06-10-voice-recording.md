# Voice Recording / Terminal Input Overlay Plan

## Goal
Add a small interaction overlay inside the terminal container that lets users type or dictate text and paste it into the terminal.

## Desired UI
- A button icon appears inside the terminal at the bottom right.
- The icon is positioned absolutely, offset from the bottom and right edges by half of `--padding`.
- Default icon: `paragraph.svg`.
- Default opacity: `30%`.
- On hover, the icon becomes fully opaque and `cursor: pointer`.

## Interaction States

### State 1: idle
- Only `paragraph.svg` is visible.
- Icon is mostly transparent (`30%` opacity).
- Hover makes it fully visible.
- Click enters the compose / voice state.

### State 2: compose ready
- `paragraph.svg` switches to `mic-mute-fill.svg`.
- A `textarea` appears, absolutely positioned at the bottom of the terminal.
- The textarea width matches the terminal width.
- The textarea is inset from the left edge, bottom edge, and right icon by `--padding`.
- A `send-fill.svg` icon appears at the right of the textarea.
- `send-fill.svg` is `30%` opaque until there is text.

### State 3: typing
- User types into the textarea.
- The textarea updates a signal on keyboard input.
- When the textarea is non-empty, `send-fill.svg` becomes fully opaque and clickable.
- Clicking `send-fill.svg` pastes the textarea content into the terminal.

### State 4: dictation
- User clicks `mic-mute-fill.svg` to start voice recording.
- Icon changes to `mic-fill.svg` while recording.
- Speech recognition writes text into the textarea.
- Clicking `mic-fill.svg` stops recording.
- `send-fill.svg` becomes active whenever the textarea is non-empty.

## Implementation Steps

1. Add a render method or node builder for the terminal overlay.
   - This should be inserted inside the terminal tab container.
   - It must be absolutely positioned inside the terminal box.

2. Define terminal overlay markup.
   - A wrapper `div` for the overlay.
   - A button/icon element for the paragraph/mic state.
   - A `textarea` element hidden by default.
   - A send icon element hidden or semi-transparent by default.

3. Add CSS rules.
   - `position: absolute; right: var(--half-padding); bottom: var(--half-padding);` for the main icon.
   - `opacity: 0.3` default, `opacity: 1` on hover/clickable states.
   - `cursor: pointer` for all active icons.
   - Textarea position: absolute bottom within terminal, left/right inset by `--padding`, and right inset enough for the icon.
   - Ensure the overlay does not disrupt terminal sizing or input focus.

4. Create signals for UI state.
   - `overlay_active: bool` (paragraph clicked or compose open)
   - `recording: bool` (microphone active)
   - `textarea_value: XString` or equivalent signal
   - `send_enabled: bool` derived from `textarea_value.is_empty()`

5. Wire events.
   - Click paragraph icon: set `overlay_active = true`, show textarea and mic icon.
   - Click mic icon when inactive: start speech recognition, set `recording = true`.
   - Click mic icon when active: stop speech recognition, set `recording = false`.
   - On textarea key input: update `textarea_value` signal.
   - Click send icon when `textarea_value` is non-empty: paste text into terminal and clear textarea.

6. Hook terminal paste behavior.
   - Use the existing terminal send/input API or work through the `TerminalJs` wrapper to inject text.
   - The send operation must call the same path as terminal input dispatch.
   - Do not use DOM event simulation or `KeyboardEvent` injection; the text must be sent through the terminal input API so the TUI receives it as real terminal input.

7. Use browser APIs for dictation.
   - Use the Web Speech API: `SpeechRecognition` or `webkitSpeechRecognition`.
   - On supported browsers, speech results should be appended to the textarea value.
   - Track recording status via `recording: bool` and switch icons between `mic-mute-fill.svg` and `mic-fill.svg`.

## Notes
- The initial button is a low-profile affordance until the user hovers or clicks it.
- The overlay should be contained entirely inside the terminal pane and not use a separate floating panel.
- The send button must only be fully active when text exists, but should still render in the compose view.
- The text must be delivered through the terminal input API; do not use low-level DOM input simulation.
- Implement the overlay in a dedicated terminal module at `terminal/src/terminal/input_overlay.rs`.

## Step-by-step priority
1. Render the base overlay node inside the terminal item view from `terminal/src/terminal/input_overlay.rs`.
2. Add CSS for absolute positioning, opacity, hover state, and textarea layout.
3. Create local state signals for overlay open/closed, textarea content, and recording status.
4. Implement click handlers for paragraph icon, mic icon, and send icon.
5. Add textarea input handling and signal updates.
6. Wire send-click to actual terminal paste.
7. Add browser speech recognition integration and recording state toggling.
8. Add a Playwright integration test in `terminal/tests/integration-test-terminal.spec.mjs`.
9. Test the full flow: open overlay, type text, send text, start/stop recording.

## Test execution
- Run the terminal integration harness with Bazel:
  `bazel test --test_output=errors --verbose_failures //terminal:terminal-integration-test-debug`
- Manual voice-recording verification is separate and should be performed in the browser when the overlay is visible.

## Expected result
A bottom-right action overlay inside each terminal tab that can show a paragraph icon, expand to a compose box with mic/send controls, and paste typed or dictated text into the terminal input.
