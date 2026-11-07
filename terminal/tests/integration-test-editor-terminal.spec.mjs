import { test, expect } from '@playwright/test';
import { mkdir, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { BASE_URL, SECOND, createTempDir, setBasePath } from './text-editor-helpers.mjs';

const panel = (page) => page.locator('.editor-terminal');
const toggle = (page) => page.locator('.toggle-terminal');
const terminal = (page) => panel(page).locator('.xterm');

async function command(page, text, expected) {
    await terminal(page).click();
    await page.keyboard.type(text);
    await page.keyboard.press('Enter');
    if (expected) await expect(terminal(page)).toContainText(expected);
}

async function openEditor(page) {
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    const tile = page.locator('.app-tile').first();
    await expect(tile).toBeVisible();
    await tile.locator('.app-menu-trigger').first().hover();
    await tile.locator('li').filter({ hasText: /^Text editor$/ }).first().click();
    await expect(toggle(page)).toBeVisible();
}

test.describe('Editor terminal', () => {
    test.beforeEach(async ({ page }) => {
        page.setDefaultTimeout(10 * SECOND);
        await openEditor(page);
    });

    test.afterEach(async ({ page }) => {
        if (await panel(page).isVisible()) {
            await toggle(page).click();
            await expect(panel(page)).toBeHidden();
        }
    });

    test('uses the base folder, restores the shell, and follows tab closure and exit', async ({ page }) => {
        test.setTimeout(90 * SECOND);
        const root = await createTempDir();
        const base = path.join(root, "folder's $(literal) space");
        await mkdir(base);
        await writeFile(path.join(base, 'file.txt'), '');
        await setBasePath(page, base, 'file.txt');
        await expect(panel(page)).toBeHidden();
        await toggle(page).click();
        await expect(panel(page)).toBeVisible();
        await command(page, 'printf "\\nEDITOR_PWD:%s\\n" "$PWD"', `EDITOR_PWD:${base}`);
        await command(page, 'cd /; export EDITOR_TOKEN=restored; echo TOKEN_READY', 'TOKEN_READY');
        await page.reload({ waitUntil: 'domcontentloaded' });
        await expect(panel(page)).toBeVisible();
        await command(page, 'printf "\\nRESTORED:%s:%s\\n" "$EDITOR_TOKEN" "$PWD"', 'RESTORED:restored:/');
        await panel(page).locator('.close-icon').click({ force: true });
        await expect(panel(page)).toBeHidden();
        await page.mouse.move(0, 0);
        await expect(toggle(page)).toHaveCSS('filter', 'invert(1)');
        await toggle(page).click();
        await command(page, 'exit');
        await expect(panel(page)).toBeHidden();
    });

    test('an early close can be ignored without losing track of the terminal', async ({ page }) => {
        let release;
        const start = new Promise((resolve) => { release = resolve; });
        // Hold PTY creation so the close request deterministically arrives first.
        await page.route('**/*add_stream*', async (route) => {
            await start;
            await route.continue();
        });
        const closed = page.waitForResponse((response) => response.url().includes('close'));
        try {
            await toggle(page).evaluate((button) => { button.click(); button.click(); });
            expect((await closed).ok()).toBeFalsy();
            await expect(panel(page)).toBeVisible();
        } finally {
            release();
        }
        await command(page, 'echo EARLY_CLOSE_CONVERGED', 'EARLY_CLOSE_CONVERGED');
        await toggle(page).click();
        await expect(panel(page)).toBeHidden();
    });

    test('failed directory initialization turns the toggle off again', async ({ page }) => {
        const base = await createTempDir();
        await writeFile(path.join(base, 'file.txt'), '');
        await setBasePath(page, base, 'file.txt');
        await rm(base, { recursive: true });
        await toggle(page).click();
        await expect(panel(page)).toBeHidden();
        await page.mouse.move(0, 0);
        await expect(toggle(page)).toHaveCSS('filter', 'invert(1)');
    });

    test('the first Terminal tile does not adopt the editor terminal', async ({ page }) => {
        const base = await createTempDir();
        await writeFile(path.join(base, 'file.txt'), '');
        await setBasePath(page, base, 'file.txt');
        await toggle(page).click();
        await command(page, 'echo EDITOR_OWNER_READY', 'EDITOR_OWNER_READY');
        const tiles = page.locator('.app-tile');
        await tiles.first().locator('.app-menu-trigger').first().hover();
        await tiles.first().locator('.split-horizontal').first().click();
        await expect(tiles).toHaveCount(2);
        const other = tiles.nth(1);
        await other.locator('.app-menu-trigger').first().hover();
        await other.locator('li').filter({ hasText: /^Terminal$/ }).first().click();
        await expect(other.locator('.add-tab-icon')).toBeVisible();
        await expect(other.locator('.xterm')).toHaveCount(0);
        await page.reload({ waitUntil: 'domcontentloaded' });
        await expect(panel(page)).toBeVisible();
        await expect(tiles.nth(1).locator('.add-tab-icon')).toBeVisible();
        await expect(tiles.nth(1).locator('.xterm')).toHaveCount(0);
    });
});
