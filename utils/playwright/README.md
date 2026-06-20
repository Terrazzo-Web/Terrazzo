# Playwright UI Access

This folder is a self-contained Playwright install for letting Codex inspect and interact with the locally running Terrazzo UI.

## Local UI

When the dev UI is already running, use:

```sh
npm run open -- http://localhost:3100
```

The local UI password is `123`.

For AI-driven inspection, use this package's Playwright dependency from `utils/playwright` and launch Chromium against `http://localhost:3100`. If a password gate appears, fill `123`.

## First-Time Setup

Install dependencies and the Chromium browser:

```sh
npm install
npx playwright install chromium
```

On macOS, launching Chromium from the sandbox can fail with a `MachPortRendezvousServer` permission error. If that happens, rerun the Playwright command with permission to launch outside the sandbox.

## Useful Commands

```sh
npm run open -- http://localhost:3100
npm run codegen -- http://localhost:3100
```
