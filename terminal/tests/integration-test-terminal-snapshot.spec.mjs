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

async function wakeTerminal(page) {
    await active(page).evaluate(element => {
        for (let node = element; node; node = node.parentElement) {
            node.dispatchEvent(new MouseEvent('mouseenter'));
        }
    });
}

test('fresh_attachment_preserves_tui_wheel_input', async ({ page, context }) => {
    test.setTimeout(90000);
    const socketPath = `/tmp/trz-mouse-${process.pid}-${Date.now()}.sock`;
    let control;
    let restored;
    try {
        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await page.locator('div.add-tab-icon img[src$="/icons/plus-square.svg"]').click();
        await expect(active(page)).toBeVisible({ timeout: 15000 });
        await expect(active(page)).toHaveAttribute("data-ready", "true", { timeout: 15000 });
        await active(page).click();
        await page.keyboard.type(`${quote(fixture)} ${quote(socketPath)}`);
        await page.keyboard.press('Enter');
        control = await connect(socketPath);
        await expect.poll(() => control.events.some(e => e.event === 'ready')).toBe(true);
        await wheel(page);
        await expect.poll(() => control.events.filter(e => e.event === 'mouse_report').length).toBeGreaterThan(0);
        const before = control.events.filter(e => e.event === 'mouse_report').length;
        await expect(active(page)).toHaveAttribute('data-checkpoint-event', /\d+/, { timeout: 15000 });
        // A later checkpoint must include the fixture's startup modes.
        const checkpoint = await active(page).getAttribute('data-checkpoint-event');
        control.socket.write('{"command":"emit_output"}\n');
        let moves = 0;
        await expect.poll(async () => {
            // Activity also exercises Resume if the browser interrupted its stream.
            const box = await active(page).boundingBox();
            if (box) { await page.mouse.move(0, 0); await page.mouse.move(box.x + 10 + (++moves % 2), box.y + 10); }
            if (await active(page).getAttribute('data-ready') === 'false') await wakeTerminal(page);
            return active(page).getAttribute('data-checkpoint-event');
        }, { timeout: 30000 }).not.toBe(checkpoint);
        await page.close();
        control.socket.write('{"command":"emit_output"}\n');
        await expect.poll(() => control.events.filter(e => e.event === 'output_complete').length, { timeout: 15000 }).toBe(2);
        restored = await context.newPage();
        restored.setDefaultTimeout(10000);
        await restored.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await expect(active(restored)).toBeVisible({ timeout: 15000 });
        await expect(active(restored)).toHaveAttribute("data-ready", "true", { timeout: 15000 });
        await expect(active(restored)).toHaveAttribute('data-output-dropped', 'true');
        await wheel(restored);
        await expect.poll(() => control.events.filter(e => e.event === 'mouse_report').length).toBeGreaterThan(before);
        const last = control.events.filter(e => e.event === 'mouse_report').at(-1);
        expect(last.button).toBe(65);
        expect(last.panel).toBe(0);
        await expect(active(restored)).toContainText(`LEFT:${last.counter}`);
    } finally {
        control?.socket.destroy();
        if (restored && !restored.isClosed()) {
            await restored.locator('div.terminals div.titles li.title img.close-icon').last().click({ force: true, timeout: 5000 }).catch(() => {});
        }
        await fs.rm(socketPath, { force: true });
    }
});

async function loadAdapter(page) {
    await page.goto('about:blank');
    await page.addScriptTag({ path: runfile('terminal/assets/jsdeps/dist/jsdeps.js') });
    const source = await fs.readFile(runfile('terminal/src/terminal/snapshot.js'), 'utf8');
    await page.addScriptTag({ content: source.replaceAll('export ', '') + '\nwindow.snapshotAdapter = { capture, restore, parserSafe, cancelPartialInput };' });
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
                const payload = snapshotAdapter.capture(original, addon, 200000);
                const restored = new JsDeps.Terminal({ cols: 80, rows: 24 });
                await snapshotAdapter.restore(restored, payload);
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
                const partial = snapshotAdapter.capture(terminal, addon, 200000);
                await write(bytes.subarray(split));
                results.push(partial === null && snapshotAdapter.capture(terminal, addon, 200000) !== null);
            }
        }
        terminal.dispose();
        return results;
    });
    expect(result.every(Boolean)).toBe(true);
});

async function loadTerminalJs(page) {
    await loadAdapter(page);
    const source = await fs.readFile(runfile('terminal/src/terminal/javascript.js'), 'utf8');
    await page.addScriptTag({ content: source
        .replace(/^import .*;\n/, 'const { capture, restore, cancelPartialInput } = window.snapshotAdapter;\n')
        .replace(/export\s*\{\s*TerminalJs\s*\};?/, '') + '\nwindow.TestTerminalJs = TerminalJs;' });
    await page.clock.install();
    await page.evaluate(async () => {
        window.captures = [];
        window.fixtureTerminal = new TestTerminalJs();
        fixtureTerminal.onCheckpoint(json => captures.push({ ...JSON.parse(json), at: performance.now() }));
        await fixtureTerminal.send(JSON.stringify({ Init: { version: 1, fresh: true, attachment: { incarnation: 'test', token: '1' }, checkpoint: null, capacity: 200000 } }));
        await fixtureTerminal.send(JSON.stringify('Ready'));
    });
    await page.clock.runFor(1);
}

test('captureThrottleKeepsPendingTrigger', async ({ page }) => {
    await loadTerminalJs(page);
    expect(await page.evaluate(() => captures.length)).toBe(1);
    await page.evaluate(async () => {
        await fixtureTerminal.checkpointFinished(true);
        await fixtureTerminal.send(JSON.stringify({ Output: { cursor: { event: '1', end: '1000' }, base64: btoa('\n'.repeat(1000)) } }));
    });
    await page.clock.runFor(4000);
    expect(await page.evaluate(() => captures.length)).toBe(1);
    await page.clock.runFor(1500);
    expect(await page.evaluate(() => captures.length)).toBe(2);
    expect(await page.evaluate(() => captures[1].cursor.event)).toBe('1');
    expect(await page.evaluate(() => captures[1].at - captures[0].at)).toBeGreaterThanOrEqual(5000);
    await page.evaluate(() => fixtureTerminal.dispose());
});

test('singleUploadRetainsPendingCapture', async ({ page }) => {
    await loadTerminalJs(page);
    await page.evaluate(async () => {
        await fixtureTerminal.send(JSON.stringify({ Output: { cursor: { event: '1', end: '1000' }, base64: btoa('\n'.repeat(1000)) } }));
    });
    await page.clock.runFor(10000);
    expect(await page.evaluate(() => captures.length)).toBe(1);
    await page.evaluate(() => fixtureTerminal.checkpointFinished(true));
    await page.clock.runFor(1);
    expect(await page.evaluate(() => captures.length)).toBe(2);
    await page.evaluate(() => fixtureTerminal.dispose());
});

test('failedUploadRetriesWithoutReplacingAcceptedSnapshot', async ({ page }) => {
    await loadTerminalJs(page);
    await page.evaluate(() => fixtureTerminal.checkpointFinished(false));
    await page.clock.runFor(4000);
    expect(await page.evaluate(() => captures.length)).toBe(1);
    await page.clock.runFor(1500);
    expect(await page.evaluate(() => captures.length)).toBe(2);
    await page.evaluate(() => fixtureTerminal.dispose());
});

test('roundTripPreservesSubsequentTerminalBehavior', async ({ page }) => {
    await loadAdapter(page);
    const results = await page.evaluate(async () => {
        const write = (terminal, data) => new Promise(resolve => terminal.write(data, resolve));
        const inspect = terminal => {
            const buffer = terminal.buffer.active;
            const lines = [];
            for (let y = 0; y < buffer.length; y++) lines.push(buffer.getLine(y).translateToString(false));
            return { lines, x: buffer.cursorX, y: buffer.cursorY, type: buffer.type, modes: terminal.modes };
        };
        const fixtures = [
            ['normal', 'hello\r\n\x1b[31mred\x1b[0m', '\r\nnext'],
            ['alternate_saved_cursor', 'shell\x1b[3;5H\x1b7\x1b[?1049hpanel', '\x1b[?1049l\x1b8X'],
            ['scroll_region_origin', '\x1b[3;20r\x1b[?6h\x1b[4;5Hinside', '\r\nmore'],
            ['hidden_cursor', '\x1b[?25lhidden', '\x1b[?25hvisible'],
            ['charset_tabs', '\x1b[3g\x1b[5G\x1bH\x1b(0lqk', '\r\tq\x1b(Btext'],
            ['wide_wrapping', '界'.repeat(45), '€\r\nnext'],
            ['application_modes', '\x1b[?1h\x1b[?66h\x1b[?2004h', 'text'],
        ];
        const results = [];
        for (const [name, setup, continuation] of fixtures) {
            const original = new JsDeps.Terminal({ cols: 80, rows: 24 });
            const restored = new JsDeps.Terminal({ cols: 80, rows: 24 });
            const addon = new JsDeps.SerializeAddon();
            original.loadAddon(addon);
            await write(original, setup);
            await snapshotAdapter.restore(restored, snapshotAdapter.capture(original, addon, 200000));
            await write(original, continuation);
            await write(restored, continuation);
            results.push({ name, original: inspect(original), restored: inspect(restored) });
            original.dispose();
            restored.dispose();
        }
        return results;
    });
    for (const result of results) expect(result.restored, result.name).toEqual(result.original);
});

test('warm_resume_preserves_applied_state', async ({ page }) => {
    await loadTerminalJs(page);
    const result = await page.evaluate(async () => {
        const original = fixtureTerminal.terminal;
        await fixtureTerminal.send(JSON.stringify({ Output: { cursor: { event: '1', end: '5' }, base64: btoa('hello') } }));
        await fixtureTerminal.send(JSON.stringify('Disconnected'));
        await fixtureTerminal.send(JSON.stringify({ Init: { version: 1, fresh: false, attachment: { incarnation: 'test', token: '2' }, checkpoint: null, capacity: 200000 } }));
        await fixtureTerminal.send(JSON.stringify('Ready'));
        const result = {
            same: original === fixtureTerminal.terminal,
            text: original.buffer.active.getLine(0).translateToString(true),
            cursor: fixtureTerminal.cursor,
            token: JSON.parse(fixtureTerminal.session()).token,
        };
        fixtureTerminal.dispose();
        return result;
    });
    expect(result).toEqual({ same: true, text: 'hello', cursor: { event: '1', end: '5' }, token: '2' });
});

test('gapResynchronizationPreservesCheckpointModes', async ({ page }) => {
    await loadAdapter(page);
    const result = await page.evaluate(async () => {
        const terminal = new JsDeps.Terminal();
        const write = data => new Promise(resolve => terminal.write(data, resolve));
        await write('\x1b[?1000h\x1b[?1006h\x1b]0;incomplete');
        const unsafe = !snapshotAdapter.parserSafe(terminal);
        snapshotAdapter.cancelPartialInput(terminal);
        await write('retained');
        const result = {
            unsafe,
            safe: snapshotAdapter.parserSafe(terminal),
            protocol: terminal.modes.mouseTrackingMode,
            encoding: terminal._core.coreMouseService.activeEncoding,
            text: terminal.buffer.active.getLine(0).translateToString(true),
        };
        terminal.dispose();
        return result;
    });
    expect(result).toEqual({ unsafe: true, safe: true, protocol: 'vt200', encoding: 'SGR', text: 'retained' });
});

test('snapshotShrinksScrollbackWithoutTruncatingVt', async ({ page }) => {
    await loadAdapter(page);
    const result = await page.evaluate(async () => {
        const original = new JsDeps.Terminal({ cols: 80, rows: 24, scrollback: 1000 });
        const addon = new JsDeps.SerializeAddon();
        original.loadAddon(addon);
        await new Promise(resolve => original.write(Array.from({ length: 1000 }, (_, i) => `${i}:${'x'.repeat(65)}\r\n`).join(''), resolve));
        const payload = snapshotAdapter.capture(original, addon, 8000);
        const restored = new JsDeps.Terminal({ cols: 80, rows: 24 });
        await snapshotAdapter.restore(restored, payload);
        const visible = terminal => Array.from({ length: 24 }, (_, i) => terminal.buffer.active.getLine(terminal.buffer.active.baseY + i).translateToString(true));
        const result = {
            bytes: new TextEncoder().encode(payload).length,
            sameVisible: JSON.stringify(visible(original)) === JSON.stringify(visible(restored)),
            tooSmall: snapshotAdapter.capture(original, addon, 1),
        };
        original.dispose();
        restored.dispose();
        return result;
    });
    expect(result.bytes).toBeLessThanOrEqual(8000);
    expect(result.sameVisible).toBe(true);
    expect(result.tooSmall).toBeNull();
});

test('continuousOutputDoesNotStarveCapture', async ({ page }) => {
    await loadTerminalJs(page);
    const result = await page.evaluate(async () => {
        await fixtureTerminal.checkpointFinished(true);
        fixtureTerminal.lastCapture = performance.now() - 5001;
        let capturedDuringOutput = false;
        for (let i = 1; i <= 8; i++) {
            await fixtureTerminal.send(JSON.stringify({ Output: { cursor: { event: String(i), end: String(i * 1000) }, base64: btoa('\n'.repeat(1000)) } }));
            if (i < 8 && captures.length > 1) capturedDuringOutput = true;
        }
        const result = { capturedDuringOutput, capturedEvent: captures[1]?.cursor.event };
        fixtureTerminal.dispose();
        return result;
    });
    expect(result).toEqual({ capturedDuringOutput: true, capturedEvent: '1' });
});

test('snapshot_transfer_interruption_can_restart_fresh', async ({ page }) => {
    await loadTerminalJs(page);
    const result = await page.evaluate(async () => {
        await fixtureTerminal.send(JSON.stringify({ Output: { cursor: { event: '1', end: '5' }, base64: btoa('hello') } }));
        const payload = JsDeps.SnapshotAdapter.capture(fixtureTerminal.terminal, fixtureTerminal.serializer, 200000);
        const checkpoint = {
            version: 1, attachment: { incarnation: 'test', token: '2' },
            cursor: { event: '1', end: '5' }, size: { r: 24, c: 80 }, payload: '',
        };
        await fixtureTerminal.send(JSON.stringify({ Init: { version: 1, fresh: true, attachment: checkpoint.attachment, checkpoint, capacity: 200000 } }));
        await fixtureTerminal.send(JSON.stringify({ SnapshotChunk: btoa('{"vt":') }));
        const partialNotReady = !fixtureTerminal.ready;
        await fixtureTerminal.send(JSON.stringify('Disconnected'));
        checkpoint.attachment = { incarnation: 'test', token: '3' };
        await fixtureTerminal.send(JSON.stringify({ Init: { version: 1, fresh: true, attachment: checkpoint.attachment, checkpoint, capacity: 200000 } }));
        const bytes = new TextEncoder().encode(payload);
        await fixtureTerminal.send(JSON.stringify({ SnapshotChunk: btoa(String.fromCharCode(...bytes)) }));
        await fixtureTerminal.send(JSON.stringify('SnapshotEnd'));
        await fixtureTerminal.send(JSON.stringify('Ready'));
        const result = { partialNotReady, ready: fixtureTerminal.ready, text: fixtureTerminal.terminal.buffer.active.getLine(0).translateToString(true) };
        fixtureTerminal.dispose();
        return result;
    });
    expect(result).toEqual({ partialNotReady: true, ready: true, text: 'hello' });
});

test('gapRequestsCheckpointEvenBelowActivityThreshold', async ({ page }) => {
    await loadTerminalJs(page);
    const result = await page.evaluate(async () => {
        await fixtureTerminal.checkpointFinished(true);
        fixtureTerminal.lastCapture = performance.now() - 5001;
        await fixtureTerminal.send(JSON.stringify({ Gap: { cursor: { event: '1', end: '999999' }, size: { r: 24, c: 80 } } }));
        const result = { count: captures.length, event: captures.at(-1).cursor.event };
        fixtureTerminal.dispose();
        return result;
    });
    expect(result).toEqual({ count: 2, event: '1' });
});

test('captureByteThresholdFitsReplayCapacity', async ({ page }) => {
    await loadTerminalJs(page);
    const result = await page.evaluate(async () => {
        await fixtureTerminal.checkpointFinished(true);
        fixtureTerminal.lastCapture = performance.now() - 5001;
        await fixtureTerminal.send(JSON.stringify({ Output: { cursor: { event: '1', end: '99999' }, base64: btoa('x'.repeat(99999)) } }));
        const before = captures.length;
        await fixtureTerminal.send(JSON.stringify({ Output: { cursor: { event: '2', end: '100000' }, base64: btoa('x') } }));
        const result = { before, after: captures.length, event: captures.at(-1).cursor.event };
        fixtureTerminal.dispose();
        return result;
    });
    expect(result).toEqual({ before: 1, after: 2, event: '2' });
});

test('warm_transport_resume_preserves_tui_wheel_input', async ({ page, context }) => {
    test.setTimeout(90000);
    const socketPath = `/tmp/trz-resume-${process.pid}-${Date.now()}.sock`;
    let control;
    try {
        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await page.locator('div.add-tab-icon img[src$="/icons/plus-square.svg"]').click();
        await expect(active(page)).toHaveAttribute('data-ready', 'true', { timeout: 15000 });
        await active(page).click();
        await page.keyboard.type(`${quote(fixture)} ${quote(socketPath)}`);
        await page.keyboard.press('Enter');
        control = await connect(socketPath);
        await expect.poll(() => control.events.some(e => e.event === 'ready')).toBe(true);
        await wheel(page);
        await expect.poll(() => control.events.filter(e => e.event === 'mouse_report').length).toBeGreaterThan(0);
        const before = control.events.filter(e => e.event === 'mouse_report').length;
        const failures = [];
        page.on('requestfailed', request => failures.push(request.url()));
        await context.setOffline(true);
        control.socket.write('{"command":"emit_output"}\n');
        await expect(active(page)).toHaveAttribute('data-ready', 'false', { timeout: 15000 });
        const failedBeforeRetry = failures.length;
        const box = await active(page).boundingBox();
        if (!box) throw new Error('Missing terminal during reconnect');
        await wakeTerminal(page);
        await expect.poll(() => failures.length).toBeGreaterThan(failedBeforeRetry);
        await context.setOffline(false);
        await page.bringToFront();
        await expect(active(page)).toHaveAttribute('data-ready', 'true', { timeout: 15000 });
        await wheel(page);
        await expect.poll(() => control.events.filter(e => e.event === 'mouse_report').length).toBeGreaterThan(before);
    } finally {
        await context.setOffline(false).catch(() => {});
        control?.socket.destroy();
        await page.locator('div.terminals div.titles li.title img.close-icon').last().click({ force: true, timeout: 5000 }).catch(() => {});
        await fs.rm(socketPath, { force: true });
    }
});
