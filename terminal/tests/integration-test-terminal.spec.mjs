import { test, expect } from '@playwright/test';

const SECOND = 1000;
const BASE_URL = process.env.BASE_URL ?? 'http://127.0.0.1:3000';

async function expectStaticAssetLoads(request, path, contentTypePattern) {
  const response = await request.get(`${BASE_URL}${path}`);
  const failureDetails = `status=${response.status()} headers=${JSON.stringify(response.headers())}`;
  expect(response.ok(), `${path} should load successfully (${failureDetails})`).toBeTruthy();
  expect(response.headers()['content-type']).toMatch(contentTypePattern);
}

function getAddTabButton(page) {
  return page.locator('div[class*="add-tab-icon-"] img');
}

function getTabs(page) {
  return page.locator(
    'div[class*=terminals] div[class*="titles-"] > ul > li[class*="title-"]:has(img[class*="close-icon-"])',
  );
}

function getActiveTerminal(page) {
  return page.locator(
    'div[class*=terminals] div[class*="items-"] > ul > li[class*="selected-"] .xterm',
  );
}

function getCloseIcons(tabs) {
  return tabs.locator('img[class*="close-icon-"]');
}

test.describe('Terminal', () => {
  test.beforeEach(async ({ page }) => {
    page.setDefaultTimeout(5 * SECOND);
    page.setDefaultNavigationTimeout(5 * SECOND);
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
  });

  test('loads /static/common.css with the expected mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/common.css', /^text\/css\b/i);
  });

  test('opens a new terminal tab and runs a command', async ({ page }) => {
    const addTabButton = getAddTabButton(page);
    await addTabButton.waitFor({ timeout: 30 * SECOND });
    await addTabButton.click();

    const tabs = getTabs(page);
    const activeTerminal = getActiveTerminal(page);
    await expect(tabs).toHaveCount(1);
    await expect(activeTerminal).toHaveCount(1);

    await activeTerminal.click();
    await page.keyboard.type('echo $((191*7))');
    await page.keyboard.press('Enter');
    await expect(activeTerminal).toContainText('1337');

    await getCloseIcons(tabs).click({ force: true });
    await expect(tabs).toHaveCount(0);
  });

  test('two terminals', async ({ page }) => {
    const addTabButton = getAddTabButton(page);
    await addTabButton.waitFor({ timeout: 30 * SECOND });
    await addTabButton.click();
    await addTabButton.click();

    const tabs = getTabs(page);
    await expect(tabs).toHaveCount(2);

    const firstTab = tabs.nth(0);
    const secondTab = tabs.nth(1);

    await firstTab.click();
    await expect(firstTab).toHaveClass(/selected-/);
    await expect(page.locator('li[class*="selected-"] .xterm')).toHaveCount(1);

    const activeTerminal = getActiveTerminal(page);
    await activeTerminal.click();
    await page.keyboard.type('echo $((191*7))');
    await page.keyboard.press('Enter');
    await expect(activeTerminal).toContainText('1337');

    await secondTab.click();
    await expect(secondTab).toHaveClass(/selected-/);
    await expect(page.locator('li[class*="selected-"] .xterm')).toHaveCount(1);

    await activeTerminal.click();
    await page.keyboard.type('echo $((191*7*2))');
    await page.keyboard.press('Enter');
    await expect(activeTerminal).toContainText('2674');
    await expect(activeTerminal).not.toContainText('1337');

    const closeIcons = getCloseIcons(tabs);
    await expect(closeIcons).toHaveCount(2);
    await closeIcons.nth(0).click({ force: true });
    await expect(tabs).toHaveCount(1);
    await closeIcons.nth(0).click({ force: true });
    await expect(tabs).toHaveCount(0);
  });
});
