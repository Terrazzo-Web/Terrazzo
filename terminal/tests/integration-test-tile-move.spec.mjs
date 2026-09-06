import { test, expect } from '@playwright/test';

const SECOND = 1000;
const BASE_URL = (process.env.BASE_URL ?? 'http://127.0.0.1:3000')
    .split(';')
    .map((url) => url.trim())
    .filter(Boolean)[0];

test.describe('Tile tabs', () => {
    test.beforeEach(async ({ page }) => {
        page.setDefaultTimeout(5 * SECOND);
        page.setDefaultNavigationTimeout(5 * SECOND);
        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await page.locator('.app-tile').first().waitFor({ timeout: 10 * SECOND });
    });

    test('moves an existing tile between tabbed arrays', async ({ page }) => {
        const tiles = page.locator('.app-tile');

        await tiles.first().locator('.app-menu-trigger').hover();
        await tiles.first().locator('.split-horizontal').click();
        await expect(tiles).toHaveCount(2);

        await tiles.nth(1).locator('.app-menu-trigger').hover();
        await tiles.nth(1).locator('.split-tabbed').click();
        await expect(page.locator('.tabbed-tile')).toHaveCount(1);

        await tiles.first().locator('.app-menu-trigger').hover();
        await tiles.first().locator('.split-tabbed').click();
        const arrays = page.locator('.tabbed-tile');
        await expect(arrays).toHaveCount(2);
        await expect(arrays.nth(0).locator('.tile-tab-title')).toHaveCount(2);
        await expect(arrays.nth(1).locator('.tile-tab-title')).toHaveCount(2);

        const destination = arrays.nth(0);
        const source = arrays.nth(1);
        const movedTitle = source.locator('.tile-tab-title').nth(1);
        const movedTitleText = await movedTitle.textContent();
        const destinationTitleTexts = await destination
            .locator('.tile-tab-title')
            .allTextContents();
        await movedTitle.click();

        // Drop after the destination's first title. Separators alternate with titles.
        const dropZone = destination
            .locator('.tile-tab-titles > ul > li:not(.tile-tab-title) > div:first-child')
            .nth(1);
        // Playwright resolves the target before beginning a native drag. Prime the
        // same drag-start state a browser sets so the destination separator exists.
        const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
        await movedTitle.dispatchEvent('dragstart', { dataTransfer });
        await expect(dropZone).toBeVisible();
        await movedTitle.dragTo(dropZone);

        await expect(destination.locator('.tile-tab-title')).toHaveCount(3);
        await expect(arrays).toHaveCount(1);
        await expect(destination.locator('.tile-tab-title')).toHaveText([
            destinationTitleTexts[0],
            movedTitleText,
            destinationTitleTexts[1],
        ]);
    });
});
