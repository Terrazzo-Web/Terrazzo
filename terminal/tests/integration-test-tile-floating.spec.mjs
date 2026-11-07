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

        const addTerminalButton = rightTile.locator(
            '.add-tab-icon img[src$="/icons/plus-square.svg"]',
        );
        await expect(addTerminalButton).toBeVisible();
        await addTerminalButton.click();
        await expect(rightTile.getByText('Terminal 1', { exact: true })).toBeVisible();
        await expect(rightTile.locator('.xterm')).toBeVisible({ timeout: 10 * SECOND });

        await initialTile.locator('.app-menu-trigger').hover();
        await initialTile.locator('li').filter({ hasText: /^Terminal$/ }).click();
        const leftAddTerminalButton = initialTile.locator(
            '.add-tab-icon img[src$="/icons/plus-square.svg"]',
        );
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

        // The right tile is nested inside another split. Maximizing must escape both.
        const signpostInTile = rightTile.locator(
            '.app-menu-trigger img[src*="signpost-split.svg"]',
        ).first();
        const originalViewport = page.viewportSize();
        const originalBounds = await rightTile.boundingBox();
        await rightTile.evaluate((node) => { window.__maximizedTile = node; });
        await signpostInTile.dblclick();
        const maximized = page.locator('body > .maximized-tile');
        await expect(maximized).toBeVisible();
        await expect(tiles).toHaveCount(3);
        const expectMargins = async () => {
            const viewport = page.viewportSize();
            await expect.poll(async () => {
                const bounds = await maximized.boundingBox();
                return bounds && Math.max(
                    Math.abs(bounds.x - viewport.width * 0.05),
                    Math.abs(bounds.y - viewport.height * 0.05),
                    Math.abs(bounds.width - viewport.width * 0.9),
                    Math.abs(bounds.height - viewport.height * 0.9),
                );
            }).toBeLessThan(1);
        };
        await expectMargins();
        await expect(maximized.locator('.xterm')).toBeVisible();
        await expect.poll(() => maximized.locator('.app-tile').evaluate(
            (node) => node === window.__maximizedTile,
        )).toBe(true);
        await page.setViewportSize({ width: 1100, height: 800 });
        await expectMargins();
        await maximized.locator('.app-menu-trigger img').first().dblclick();
        await expect(maximized).toHaveCount(0);
        await expect.poll(() => xterms.evaluateAll((nodes) => (
            nodes.every((node) => window.__xtermsBeforeSplit.includes(node))
        ))).toBe(true);
        await expect(rightTile).toBeVisible();
        await page.setViewportSize(originalViewport);
        await expect.poll(async () => {
            const bounds = await rightTile.boundingBox();
            return Math.max(...['x', 'y', 'width', 'height'].map(
                (key) => Math.abs(bounds[key] - originalBounds[key]),
            ));
        }).toBeLessThan(1);

        const newTile = tiles.filter({ hasNot: page.locator('.xterm') });
        await signpostInTile.dblclick();
        await newTile.locator('.app-menu-trigger').dispatchEvent('mouseover');
        await newTile.locator('.tile-close').dispatchEvent('click');
        await expect(tiles).toHaveCount(2);
        await expect(maximized.locator('.xterm')).toBeVisible();
        await maximized.locator('.app-menu-trigger img').first().dblclick();
        await expect(maximized).toHaveCount(0);

        await rightTile.locator('.app-menu-trigger').hover();
        await signpostInTile.dblclick();
        await expect(maximized).toBeVisible();
        await page.reload({ waitUntil: 'domcontentloaded' });
        await expect(maximized).toHaveCount(0);
        await expect(tiles).toHaveCount(2);
        await expect(rightTile.locator('.xterm')).toBeVisible();
        // Add the other split direction to exercise clipping by mixed nested ancestors.
        await rightTile.locator('.app-menu-trigger').hover();
        await rightTile.locator('.split-vertical').click();
        await expect(tiles).toHaveCount(3);
        await rightTile.locator('.xterm').evaluate((node) => {
            window.__nestedXterm = node;
        });
        await signpostInTile.dblclick();
        await expectMargins();
        await expect(maximized.locator('.xterm')).toBeVisible();
        await maximized.locator('.app-menu-trigger img').first().dblclick();
        await expect(maximized).toHaveCount(0);
        await expect.poll(() => rightTile.locator('.xterm').evaluate(
            (node) => node === window.__nestedXterm,
        )).toBe(true);
        await tiles.filter({ hasNot: page.locator('.xterm') })
            .locator('.app-menu-trigger').hover();
        await tiles.filter({ hasNot: page.locator('.xterm') })
            .locator('.tile-close').click();
        await expect(tiles).toHaveCount(2);
        await signpostInTile.dblclick();
        await maximized.locator('.app-menu-trigger').hover();
        await maximized.locator('.float-tile').dispatchEvent('click');
        await expect(maximized).toHaveCount(0);

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

        // A docked tile under the tabbed floating host can maximize independently.
        const dockedTile = page.locator('.app-tile:not(.floating-tile .app-tile)').first();
        await dockedTile.locator('.app-menu-trigger img').first().dispatchEvent('dblclick');
        await expectMargins();
        await maximized.locator('.app-menu-trigger img').first().dblclick();
        await expect(maximized).toHaveCount(0);
        await expect(floatingTile).toHaveCSS('height', '600px');
    });
});
