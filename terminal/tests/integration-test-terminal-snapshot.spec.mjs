import { test, expect } from '@playwright/test';
import net from 'node:net';
import path from 'node:path';
import fs from 'node:fs/promises';

const BASE_URL = (process.env.BASE_URL ?? 'http://127.0.0.1:3000').split(';')[0];
const runfile = name => path.join(process.env.TEST_SRCDIR, process.env.TEST_WORKSPACE, name);
const fixture = runfile('terminal/integration/mouse-tui');
test.beforeEach(async ({ page }) => { page.setDefaultTimeout(10000); page.on("pageerror", error => console.error("Browser error:", error.message)); page.on("console", msg => { if (msg.type() === "warning" || msg.type() === "error") console.log("Browser console:", msg.text()); }); });
const quote = text => "'" + text.replaceAll("'", "'\\''") + "'";
const active = page => page.locator('div.terminals div.items > ul > li.selected .xterm');

async function connect(socketPath) {
    for (let attempt = 0; attempt < 100; attempt++) {
        const socket = net.createConnection(socketPath);
        const connected = await new Promise(resolve => {
            socket.once('connect', () => resolve(true));
            socket.once('error', () => resolve(false));
        });
        if (connected) {
            const events = [];
            let pending = '';
            socket.on('data', data => {
                pending += data;
                const lines = pending.split('\n');
                pending = lines.pop();
                for (const line of lines) events.push(JSON.parse(line));
            });
            return { socket, events };
        }
        socket.destroy();
        await new Promise(resolve => setTimeout(resolve, 50));
    }
    throw new Error('Fixture did not create its test socket');
}

async function wheel(page) {
    const screen = active(page).locator('.xterm-screen');
    const box = await screen.boundingBox();
    if (!box) throw new Error('Terminal screen is missing');
    await Promise.race([
        page.mouse.move(box.x + 30, box.y + 30),
        new Promise((_, reject) => setTimeout(() => reject(new Error('Browser mouse move stalled')), 5000)),
    ]);
    await Promise.race([
        page.mouse.wheel(0, 120),
        new Promise((_, reject) => setTimeout(() => reject(new Error('Browser wheel dispatch stalled')), 5000)),
    ]);
}

async function openPage(page) {
    // Wait for the restored terminal tile to request its own selected-tab state.
    const initialized = new Set();
    const onRequest = request => {
        if (/\/api\/fn\/get\d+$/.test(request.url())) {
            const tile = request.postDataJSON()?.tile;
            if (tile !== undefined) initialized.add(tile);
        }
    };
    page.on('request', onRequest);
    const tiles = page.waitForResponse(async response => {
        if (!/\/api\/fn\/get\d+$/.test(response.url())) return false;
        return !!(await response.json().catch(() => null))?.Tile;
    });
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    const tree = await (await tiles).json();
    await expect.poll(() => initialized.has(tree.Tile.id)).toBe(true);
    page.off('request', onRequest);
}

function nextSnapshot(page, predicate) {
    return page.waitForResponse(response => {
        if (!response.url().includes('/api/fn/put_snapshot') || !response.ok()) return false;
        return predicate(response.request().postDataJSON().snapshot);
    }, { timeout: 20000 });
}

test('fresh_open_restores_snapshot_and_bounded_delta_with_mouse_modes', async ({ page, context }) => {
    test.setTimeout(90000);
    const socketPath = `/tmp/trz-snapshot-${process.pid}-${Date.now()}.sock`;
    let control;
    let reopened;
    try {
        await openPage(page);
        await page.locator('div.add-tab-icon img[src$="/icons/plus-square.svg"]').click();
        await expect(active(page)).toBeVisible();
        const saved = nextSnapshot(page, snapshot => JSON.parse(snapshot.data).extra.encoding === 'SGR');
        await active(page).click();
        await page.keyboard.type(`${quote(fixture)} ${quote(socketPath)}`);
        await page.keyboard.press('Enter');
        control = await connect(socketPath);
        await expect.poll(() => control.events.some(event => event.event === 'ready')).toBe(true);
        await wheel(page);
        await expect.poll(() => control.events.filter(event => event.event === 'mouse_report').length).toBeGreaterThan(0);
        await saved;
        await page.close();
        // Flood the disconnected tail beyond both the debug and release capacities.
        control.socket.write('{"command":"emit_output"}\n');
        await expect.poll(() => control.events.filter(event => event.event === 'output_complete').length).toBe(1);
        reopened = await context.newPage();
        await openPage(reopened);
        await expect(active(reopened)).toContainText('LEFT:', { timeout: 20000 });
        const before = control.events.filter(event => event.event === 'mouse_report').length;
        await wheel(reopened);
        await expect.poll(() => control.events.filter(event => event.event === 'mouse_report').length).toBeGreaterThan(before);
        const report = control.events.filter(event => event.event === 'mouse_report').at(-1);
        expect(report.button).toBe(65);
        expect(report.panel).toBe(0);
    } finally {
        control?.socket.destroy();
        if (reopened && !reopened.isClosed()) {
            await reopened.locator('img.close-icon').last().click({ force: true }).catch(() => {});
        }
        await fs.rm(socketPath, { force: true });
    }
});

async function loadAdapter(page) {
    await page.setContent('<!doctype html><html><body></body></html>');
    await page.addScriptTag({ path: runfile('terminal/assets/jsdeps/dist/jsdeps.js') });
}

test('roundTripPreservesMouseReports', async ({ page }) => {
    await loadAdapter(page);
    const results = await page.evaluate(async () => {
        const results = [];
        const write = (terminal, data) => new Promise(resolve => terminal.write(data, resolve));
        for (const tracking of [9, 1000, 1002, 1003]) {
            for (const encoding of [0, 1006, 1016]) {
                const original = new JsDeps.Terminal({ cols: 80, rows: 24 });
                const addon = new JsDeps.SerializeAddon();
                original.loadAddon(addon);
                await write(original, `\x1b[?1049h\x1b[?${tracking}h${encoding ? `\x1b[?${encoding}h` : ""}panel`);
                const payload = JsDeps.SnapshotAdapter.capture(original, addon, 200000);
                const restored = new JsDeps.Terminal({ cols: 80, rows: 24 });
                await JsDeps.SnapshotAdapter.restore(restored, payload);
                const reports = [];
                for (const terminal of [original, restored]) {
                    let report = '';
                    terminal.onData(data => { report += data; });
                    terminal.onBinary(data => { report += data; });
                    terminal._core.coreMouseService.triggerMouseEvent({ col: 2, row: 2, x: 20, y: 20, button: 4, action: 0, ctrl: false, alt: false, shift: false });
                    reports.push(report);
                }
                results.push({ tracking, encoding, reports, modesEqual: original.modes.mouseTrackingMode === restored.modes.mouseTrackingMode });
                original.dispose();
                restored.dispose();
            }
        }
        return results;
    });
    for (const result of results) {
        expect(result.modesEqual).toBe(true);
        expect(result.reports[1]).toBe(result.reports[0]);
        if (result.tracking !== 9) expect(result.reports[0]).not.toBe('');
    }
});

test('captureWaitsForSafeParserBoundary', async ({ page }) => {
    await loadAdapter(page);
    const result = await page.evaluate(async () => {
        const terminal = new JsDeps.Terminal();
        const addon = new JsDeps.SerializeAddon();
        terminal.loadAddon(addon);
        const write = data => new Promise(resolve => terminal.write(data, resolve));
        const results = [];
        for (const bytes of [new TextEncoder().encode('€'), new TextEncoder().encode('\x1b[?1006h'), new TextEncoder().encode('\x1b]0;title\x07'), new TextEncoder().encode('\x1bP$qm\x1b\\')]) {
            for (let split = 1; split < bytes.length; split++) {
                terminal.reset();
                await write(bytes.subarray(0, split));
                const partial = JsDeps.SnapshotAdapter.capture(terminal, addon, 200000);
                await write(bytes.subarray(split));
                results.push(partial === null && JsDeps.SnapshotAdapter.capture(terminal, addon, 200000) !== null);
            }
        }
        terminal.dispose();
        return results;
    });
    expect(result.every(Boolean)).toBe(true);
});

test('periodic_upload_is_serial_and_stops_on_dispose', async ({ page }) => {
    await loadAdapter(page);
    const source = await fs.readFile(runfile('terminal/src/terminal/javascript.js'), 'utf8');
    await page.addScriptTag({ content: source.replace(/export\s*\{\s*TerminalJs\s*\};?/, '') + '\nwindow.TestTerminalJs = TerminalJs;' });
    await page.evaluate(async () => {
        window.widget = new TestTerminalJs();
        await widget.initialize(7, 200000, 'null');
        await widget.send(new TextEncoder().encode('hello'), 1);
    });
    await page.clock.install();
    await page.evaluate(() => {
        window.captures = [];
        widget.onSnapshot(json => {
            captures.push(JSON.parse(json));
            return captures.length === 1 ? new Promise(resolve => { window.finishUpload = resolve; }) : Promise.resolve();
        });
    });
    await page.clock.runFor(5000);
    expect(await page.evaluate(() => captures.length)).toBe(1);
    await page.evaluate(() => { window.writing = widget.send(new TextEncoder().encode(' delta'), 2); });
    await page.clock.runFor(1);
    await page.evaluate(() => writing);
    await page.clock.runFor(10000);
    expect(await page.evaluate(() => captures.length)).toBe(1);
    await page.evaluate(() => finishUpload());
    await page.clock.runFor(5000);
    expect(await page.evaluate(() => captures.map(snapshot => snapshot.position))).toEqual([1, 2]);
    await page.evaluate(() => widget.dispose());
    await page.clock.runFor(20000);
    expect(await page.evaluate(() => captures.length)).toBe(2);
});
