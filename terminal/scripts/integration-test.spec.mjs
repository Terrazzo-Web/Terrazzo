import { test, expect } from '@playwright/test';

const SECOND = 1000;
const BASE_URL = process.env.BASE_URL ?? 'http://127.0.0.1:3000';

async function expectStaticAssetLoads(request, path, contentTypePattern) {
  const response = await request.get(`${BASE_URL}${path}`);
  const failureDetails = `status=${response.status()} headers=${JSON.stringify(response.headers())}`;
  expect(response.ok(), `${path} should load successfully (${failureDetails})`).toBeTruthy();
  expect(response.headers()['content-type']).toMatch(contentTypePattern);
}

async function fetchServerFnFromPage(page, path, payload) {
  return page.evaluate(async ({ path, payload }) => {
    const response = await fetch(path, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(payload),
      credentials: 'same-origin',
    });
    return {
      status: response.status,
      contentType: response.headers.get('content-type'),
      body: await response.text(),
    };
  }, { path, payload });
}

test.describe('Converter', () => {
  test.beforeEach(async ({ page }) => {
    page.setDefaultTimeout(5 * SECOND);
    page.setDefaultNavigationTimeout(5 * SECOND);
    await page.goto(BASE_URL, { waitUntil: 'networkidle' });
  });

  test('loads /static/common.css with the expected mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/common.css', /^text\/css\b/i);
  });

  test('reports the current response for /api/fn/get2462584562250403446', async ({ page }) => {
    const response = await fetchServerFnFromPage(
      page,
      '/api/fn/get2462584562250403446',
      { remote: null },
    );
    expect(response.status).toBe(400);
    expect(response.body).toContain(
      'Could not find a server function at the route /api/fn/get2462584562250403446.',
    );
  });

  test('typing 123 shows 123 in the selected conversion panel', async ({ page }) => {
    await page.locator('.app-menu-trigger').hover();
    await page.getByText('Converter', { exact: true }).click();

    const input = page.locator('textarea.converter-input');
    await expect(input).toBeVisible();
    await page.waitForTimeout(500);
    const conversionsResponse = page.waitForResponse((response) =>
      response.request().method() === 'POST' &&
      response.url().includes('/api/fn/get_conversions'),
    );
    await input.click();
    await input.pressSequentially('123');
    expect((await conversionsResponse).ok()).toBeTruthy();

    await expect(page.locator('pre.converter-output').first()).toHaveText('123');
  });
});
