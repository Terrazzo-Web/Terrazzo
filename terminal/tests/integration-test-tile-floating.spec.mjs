import { test, expect } from '@playwright/test';

const SECOND = 1000;
const BASE_URL = (process.env.BASE_URL ?? 'http://127.0.0.1:3000')
    .split(';')
    .map((url) => url.trim())
    .filter(Boolean)[0];

test.describe('Floating terminal tile', () => {
    test.beforeEach(async ({ page }) => {
        page.setDefaultTimeout(5 * SECOND);
        page.setDefaultNavigationTimeout(5 * SECOND);
        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });

        const password = page.locator('input[type="password"]');
        if (await password.isVisible().catch(() => false)) {
            await password.fill('123');
            await password.dispatchEvent('change');
        }

        await page.locator('.app-tile').first().waitFor({ timeout: 10 * SECOND });
    });

    test('preserves populated tiles when splitting and floating', async ({ page }) => {
        const tiles = page.locator('.app-tile');
        const initialTile = tiles.first();

        await initialTile.locator('.app-menu-trigger').hover();
        await initialTile.locator('.split-horizontal').click();
        await expect(tiles).toHaveCount(2);

        const rightTile = tiles.nth(1);
        await rightTile.locator('.app-menu-trigger').hover();
        await rightTile.locator('li').filter({ hasText: /^Terminal$/ }).click();

        const addTerminalButton = rightTile.locator('.add-tab-icon img');
        await expect(addTerminalButton).toBeVisible();
        await addTerminalButton.click();
        await expect(rightTile.getByText('Terminal 1', { exact: true })).toBeVisible();
        await expect(rightTile.locator('.xterm')).toBeVisible({ timeout: 10 * SECOND });

        await initialTile.locator('.app-menu-trigger').hover();
        await initialTile.locator('li').filter({ hasText: /^Terminal$/ }).click();
        const leftAddTerminalButton = initialTile.locator('.add-tab-icon img');
        await expect(leftAddTerminalButton).toBeVisible();
        await leftAddTerminalButton.click();
        await expect(initialTile.locator('.xterm')).toBeVisible({ timeout: 10 * SECOND });

        const xterms = page.locator('.xterm');
        await expect(xterms).toHaveCount(2);
        await tiles.evaluateAll((nodes) => {
            window.__tilesBeforeSplit = nodes;
        });
        await xterms.evaluateAll((nodes) => {
            window.__xtermsBeforeSplit = nodes;
        });

        await rightTile.locator('.app-menu-trigger').hover();
        await rightTile.locator('.split-horizontal').click();
        await expect(tiles).toHaveCount(3);
        await expect(xterms).toHaveCount(2);
        await expect.poll(() => tiles.evaluateAll((nodes) => (
            window.__tilesBeforeSplit.every((node) => nodes.includes(node))
        ))).toBe(true);
        await expect.poll(() => xterms.evaluateAll((nodes) => (
            nodes.length === window.__xtermsBeforeSplit.length
            && nodes.every((node) => window.__xtermsBeforeSplit.includes(node))
        ))).toBe(true);

        const newTile = tiles.nth(2);
        await newTile.locator('.app-menu-trigger').hover();
        await newTile.locator('.tile-close').click();
        await expect(tiles).toHaveCount(2);

        await rightTile.locator('.app-menu-trigger').hover();
        await rightTile.locator('.float-tile').dispatchEvent('click');

        const floatingTile = page.locator('.floating-tile');
        await expect(floatingTile).toBeVisible({ timeout: 10 * SECOND });
        await expect(floatingTile).toHaveCSS('width', '800px');
        await expect(floatingTile).toHaveCSS('height', '600px');
        await expect(floatingTile.getByText('Terminal 1', { exact: true })).toBeVisible();
        await expect(floatingTile.locator('.xterm')).toBeVisible();
        await expect(page.locator('.tabbed-tile')).toBeVisible();

        const signpost = floatingTile.locator(
            '.app-menu-trigger img[src*="signpost-split.svg"]',
        ).first();
        await signpost.dblclick();

        await expect(floatingTile).toHaveCSS('height', '32px');
        await expect(signpost).toBeVisible();
        await expect(floatingTile.getByText('Terminal 1', { exact: true })).toBeVisible();
        await expect(floatingTile.locator('[class*="app-collapsible-content-"]')).toBeHidden();
        await expect(floatingTile.locator('.xterm')).toBeHidden();

        await signpost.dblclick();
        await expect(floatingTile).toHaveCSS('height', '600px');
        await expect(floatingTile.locator('.xterm')).toBeVisible();
    });
});
