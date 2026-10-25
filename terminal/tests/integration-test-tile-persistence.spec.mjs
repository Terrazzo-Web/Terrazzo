import { expect, test } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { SECOND, getCodeMirrorContent, openFolderFile, setBasePath } from './text-editor-helpers.mjs';

// Own the process and config so restarting cannot disrupt the harness or other tests.
async function startServer(configFile, endpointFile, request) {
    await rm(endpointFile, { force: true });
    const child = spawn(process.env.TERRAZZO_SERVER_BIN, [
        '--config-file', configFile,
        '--set-current-endpoint', endpointFile,
    ], { env: process.env, stdio: ['ignore', 'pipe', 'pipe'] });
    let output = '';
    let startupError;
    child.stdout.on('data', (chunk) => { output = (output + chunk).slice(-30000); });
    child.stderr.on('data', (chunk) => { output = (output + chunk).slice(-30000); });
    child.on('error', (error) => { startupError = error; });
    const closed = new Promise((resolve) => child.once('close', resolve));
    const stop = async () => {
        if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
        await closed;
    };
    let url;
    try {
        await expect.poll(async () => {
            if (startupError) throw startupError;
            if (child.exitCode !== null || child.signalCode !== null) {
                throw new Error(`Server exited during startup:\n${output}`);
            }
            const endpoints = await readFile(endpointFile, 'utf8').catch(() => '');
            const endpoint = endpoints.split(';')[0].trim();
            if (!endpoint) return false;
            url = `http://${endpoint}`;
            return request.get(url, { timeout: SECOND }).then((response) => response.ok()).catch(() => false);
        }, { timeout: 30 * SECOND, message: 'isolated server should become ready' }).toBe(true);
    } catch (error) {
        await stop();
        throw new Error(`${error.message}\nServer output:\n${output}`);
    }
    return { url, stop };
}

function tiles(tree) {
    if (tree.Tile) return [tree.Tile];
    return [
        ...tree.Array.nodes.flatMap(tiles),
        ...(tree.Array.floating_nodes ?? []).flatMap((floating) => tiles(floating.tile)),
    ];
}

async function readTree(file) {
    return JSON.parse(await readFile(file, 'utf8'));
}

async function expectSavedPath(file, base, name) {
    await expect.poll(async () => {
        const tree = await readTree(file).catch(() => null);
        if (!tree) return null;
        const editor = tiles(tree).find((tile) => ['TextEditor', 'E'].includes(tile.app));
        const saved = editor?.app_config?.editor_path;
        if (!saved) return null;
        // FilePath uses short field names in builds without diagnostics.
        return { base: saved.base ?? saved.b, file: (saved.file ?? saved.f).replace(/^\/+/, '') };
    }, { timeout: 10 * SECOND }).toEqual({ base, file: name });
}

test('restores tiles and the updated editor FilePath after a server restart', async ({ browser, request }) => {
    test.setTimeout(120 * SECOND);
    expect(process.env.TERRAZZO_SERVER_BIN, 'TERRAZZO_SERVER_BIN must point to a text-editor server').toBeTruthy();
    const dir = await mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'tile-persistence-'));
    const configFile = path.join(dir, 'config.toml');
    const tilesFile = path.join(dir, 'tiles.json');
    const endpointFile = path.join(dir, 'endpoint');
    const initialBase = path.join(dir, 'initial');
    const updatedBase = path.join(dir, 'updated');
    let server;
    let context;
    try {
        await mkdir(initialBase);
        await mkdir(updatedBase);
        await writeFile(path.join(initialBase, 'first.txt'), 'Initial editor file');
        await writeFile(path.join(updatedBase, 'second.txt'), 'Restored editor file');
        // Omit tiles_file to also exercise the default beside the config file.
        await writeFile(configFile, `[server]\nhost = "localhost"\nports = [0]\nprivate_root_ca = ${JSON.stringify(path.join(dir, 'root-ca'))}\npidfile = ${JSON.stringify(path.join(dir, 'pid'))}\n`);
        server = await startServer(configFile, endpointFile, request);
        await request.post(`${server.url}/api/login`, { data: null });
        const asset = await request.get(`${server.url}/static/wasm/terrazzo_terminal_bg.wasm`);
        const served = await asset.body();
        const built = await readFile(path.join(process.env.CARGO_MANIFEST_DIR, 'target/assets/wasm/terrazzo_terminal_bg.wasm'));
        console.log('Asset', served.length, built.length, served.equals(built), served.includes(Buffer.from('set_editor_path')));
        context = await browser.newContext();
        let page = await context.newPage();
        page.on('console', (message) => { if (/Register persistence|Persist editor|Failed to save editor/.test(message.text())) console.log(message.text()); });
        page.on('request', (request) => { if (request.url().includes('set_editor_path')) console.log('SAVE', request.postData()); });
        page.setDefaultTimeout(10 * SECOND);
        page.setDefaultNavigationTimeout(10 * SECOND);
        await page.goto(server.url, { waitUntil: 'domcontentloaded' });
        const editorTile = page.locator('.app-tile').first();
        await editorTile.locator('.app-menu-trigger').hover();
        await editorTile.locator('.split-tabbed').click();
        await expect(page.locator('.tile-tab-title')).toHaveCount(2);
        await page.locator(".add-tile-tab > div").click();
        await expect(page.locator('.tile-tab-title')).toHaveCount(3);
        await page.locator('.tile-tab-title').first().click();

        await setBasePath(page, initialBase, 'first.txt');
        await openFolderFile(page, 'first.txt');
        await expect(getCodeMirrorContent(page)).toContainText('Initial editor file');
        await expectSavedPath(tilesFile, initialBase, 'first.txt');
        const initialTree = await readTree(tilesFile);
        expect(tiles(initialTree)).toHaveLength(3);
        expect(tiles(initialTree).filter((tile) => ['TextEditor', 'E'].includes(tile.app))).toHaveLength(1);

        await setBasePath(page, updatedBase, 'second.txt');
        await openFolderFile(page, 'second.txt');
        await expect(getCodeMirrorContent(page)).toContainText('Restored editor file');
        await expectSavedPath(tilesFile, updatedBase, 'second.txt');
        const savedTree = await readTree(tilesFile);
        expect(savedTree).not.toEqual(initialTree);
        const titles = await page.locator('.tile-tab-title').allTextContents();

        await server.stop();
        await context.close();
        context = undefined;
        server = await startServer(configFile, endpointFile, request);
        // A fresh context rules out client memory or browser storage restoring the state.
        context = await browser.newContext();
        page = await context.newPage();
        await page.goto(server.url, { waitUntil: 'domcontentloaded' });
        await expect(page.locator('.tile-tab-title')).toHaveText(titles);
        await expect(getCodeMirrorContent(page)).toContainText('Restored editor file');
        await expect(page.locator('.base-path-selector-display')).toHaveText(updatedBase);
        await expect(page.locator('.file-path-selector-field')).toHaveValue(/(^|\/)second\.txt$/);
        await expect.poll(() => readTree(tilesFile)).toEqual(savedTree);
    } catch (error) {
        console.log("Saved tree:", await readFile(tilesFile, "utf8").catch(() => "missing"));
        throw error;
    } finally {
        await server?.stop();
        await context?.close().catch(() => {});
        await rm(dir, { recursive: true, force: true });
    }
});
