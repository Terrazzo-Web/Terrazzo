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
        await page.locator('.app-tile').first().waitFor({ timeout: 10 * SECOND });
    });

    test('renders immediately after floating a populated split tile', async ({ page }) => {
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

        await rightTile.locator('.app-menu-trigger').hover();
        await rightTile.locator('.float-tile').click();

        const floatingTile = page.locator('.floating-tile');
        await expect(floatingTile).toBeVisible();
        await expect(floatingTile).toHaveCSS('width', '800px');
        await expect(floatingTile).toHaveCSS('height', '600px');
        await expect(floatingTile.getByText('Terminal 1', { exact: true })).toBeVisible();
        await expect(floatingTile.locator('.xterm')).toBeVisible();
        await expect(page.locator('.tabbed-tile')).toBeVisible();
    });
});
