import { test, expect } from '@playwright/test';

const SECOND = 1000;
const BASE_URL = process.env.BASE_URL ?? 'http://127.0.0.1:3000';

async function expectStaticAssetLoads(request, path, contentTypePattern) {
  const response = await request.get(`${BASE_URL}${path}`);
  const failureDetails = `status=${response.status()} headers=${JSON.stringify(response.headers())}`;
  expect(response.ok(), `${path} should load successfully (${failureDetails})`).toBeTruthy();
  expect(response.headers()['content-type']).toMatch(contentTypePattern);
}

test.describe('Terminal', () => {
  test.beforeEach(async ({ page }) => {
    page.setDefaultTimeout(5 * SECOND);
    page.setDefaultNavigationTimeout(5 * SECOND);
    await page.goto(BASE_URL, { waitUntil: 'networkidle' });
  });

  test('loads /static/common.css with the expected mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/common.css', /^text\/css\b/i);
  });

  test('opens a new terminal tab and runs a command', async ({ page }) => {
    await page.locator('div[class^="add-tab-icon-"] img').waitFor({ timeout: 30 * SECOND });
    await page.locator('div[class^="add-tab-icon-"] img').click();

    await expect(page.locator('.xterm')).toHaveCount(1);

    const activeTerminal = page.locator('.xterm').first();
    await activeTerminal.click();
    await page.keyboard.type('echo $((191*7))');
    await page.keyboard.press('Enter');

    await expect(activeTerminal).toContainText('1337');
  });
});
