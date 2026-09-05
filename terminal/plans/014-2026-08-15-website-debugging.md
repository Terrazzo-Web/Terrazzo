# AI Website Debugging Playbook

Use this playbook to compile the client, run the local server, and inspect the
website with Playwright.

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

Use the Playwright dependency and browser environment prepared by Bazel. Do not
install a separate Playwright copy.

Prepare Bazel's Playwright bundle, create an isolated working directory, and
run an ad hoc spec against the server already running on port 3100:

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
event before waiting for the application UI. A minimal setup is:

```js
import { expect, test } from '@playwright/test';

const baseUrl = process.env.BASE_URL ?? 'http://127.0.0.1:3100';

test('inspect the local website', async ({ page }) => {
    await page.goto(baseUrl, { waitUntil: 'domcontentloaded' });

    const password = page.locator('input[type="password"]');
    if (await password.isVisible().catch(() => false)) {
        await password.fill('123');
        await password.dispatchEvent('change');
    }

    await expect(page.locator('.app-menu-trigger').first()).toBeVisible({
        timeout: 10_000,
    });

    // Continue inspecting the page with Playwright locators and assertions.
});
```

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
