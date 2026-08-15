# Website Debugging

## Start the local server

Cargo does not always notice the client-side source changes that must trigger a
new website build. Touch the terminal crate entry point before starting the
server:

```sh
touch terminal/src/lib.rs
./terminal/tests/run-server.sh
```

The test configuration serves the website at `http://localhost:3100`. If the
login screen appears, use the local test password `123`.

When changing another crate, also touch that crate's entry point before
restarting the server. For example, after changing the framework client, use:

```sh
touch framework/client/src/lib.rs
touch terminal/src/lib.rs
./terminal/tests/run-server.sh
```

Without these `touch` commands, Cargo can consider the generated client build
up to date and skip recompiling the website.

## Use the Bazel Playwright environment

Use the same Playwright dependency and browser environment as the Bazel
integration tests. Follow the test structure and selectors in
`terminal/tests/integration-test-terminal.spec.mjs` and the other
`integration-test-*.spec.mjs` files.

For a checked-in integration test, add or update its target in
`terminal/BUILD.bazel`, then run the debug target through Bazel. For example:

```sh
bazel test //terminal:terminal-integration-test-debug \
  --test_output=streamed
```

For an ad hoc Playwright spec against the server already running on port 3100,
prepare and reuse Bazel's Playwright bundle instead of installing a separate
copy:

```sh
bazel build //bazel:playwright_setup
playwright_root="$(realpath bazel-bin/bazel/playwright_setup)"
playwright_work="$(mktemp -d)"
ln -s "$playwright_root/node_modules" "$playwright_work/node_modules"
ln -s "$playwright_root/package.json" "$playwright_work/package.json"
cp path/to/debug-website.spec.mjs "$playwright_work/"
cd "$playwright_work"
HOME="$playwright_root/home" \
TMPDIR="$playwright_work" \
BAZEL=1 \
BASE_URL=http://127.0.0.1:3100 \
"$playwright_root/node_modules/.bin/playwright" test \
  debug-website.spec.mjs --reporter=line --workers=1
```

The debug spec can use the same login behavior as the site: wait for
`input[type="password"]`, fill `123` if it is visible, and dispatch its `change`
event before waiting for the application UI.

## Signpost menu check

The menu trigger is `.app-menu-trigger`, and its image loads
`signpost-split.svg`. Hover the image and assert that the menu becomes visible:

```js
const trigger = page.locator('.app-menu-trigger').first();
const signpost = trigger.locator('img[src*="signpost-split.svg"]');

await expect(signpost).toBeVisible();
await signpost.hover();
await expect(page.locator('ul').filter({
    has: page.locator('.split-horizontal'),
}).first()).toBeVisible();
```

Verified menu options:

- Applications:
  - Terminal
  - Text editor
  - Converter
  - Port forward
- Tile actions:
  - Split horizontally (`.split-horizontal`)
  - Split vertically (`.split-vertical`)
  - Convert to tabs (`.split-tabbed`)
  - Float the tile (`.float-tile`)
  - Close the tile (`.tile-close`)

This list was confirmed against the locally running debug server using the
Bazel-provisioned Playwright environment; all nine options were visible after
hovering `signpost-split.svg`.
