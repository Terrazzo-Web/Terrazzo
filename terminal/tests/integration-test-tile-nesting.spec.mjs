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

    test('preserves nested tab groups when tabifying an existing tab', async ({ page }) => {
        const tile = page.locator('.app-tile').first();
        await tile.locator('.app-menu-trigger').hover();
        await tile.locator('.split-tabbed').click();
        await expect(page.locator('.tabbed-tile')).toHaveCount(1);

        const parentTitles = page.locator('.tile-tab-titles').first()
            .locator(':scope > ul > .tile-tab-title');
        await expect(parentTitles).toHaveCount(2);
        const siblingTitle = await parentTitles.nth(1).textContent();

        await tile.locator('.app-menu-trigger').hover();
        await tile.locator('.split-tabbed').click();

        await expect(page.locator('.tabbed-tile')).toHaveCount(2);
        await expect(parentTitles).toHaveCount(2);
        const nestedGroup = page.locator('.tabbed-tile .tabbed-tile');
        await expect(nestedGroup).toHaveCount(1);
        const nestedTitles = nestedGroup.locator('.tile-tab-title');
        await expect(nestedTitles).toHaveCount(2);
        await nestedTitles.nth(1).click();
        await expect(nestedGroup.locator('.app-tile').nth(0)).toBeHidden();
        await expect(nestedGroup.locator('.app-tile').nth(1)).toBeVisible();

        await parentTitles.nth(1).click();
        await expect(parentTitles.nth(1)).toHaveText(siblingTitle);
        await expect(nestedGroup).toBeHidden();
        await parentTitles.nth(0).click();
        await expect(nestedGroup).toBeVisible();
        await expect(nestedTitles).toHaveCount(2);
    });
});
