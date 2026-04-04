import { test, expect } from '@playwright/test';

const SECOND = 1000;
const BASE_URL = 'http://127.0.0.1:3000';

function counterControls(page) {
  const counter = page.locator('#counter');
  return {
    incrementButton: counter.getByRole('button', { name: '+1', exact: true }),
    decrementButton: counter.getByRole('button', { name: '-1', exact: true }),
    clearButton: counter.getByRole('button', { name: 'Clear', exact: true }),
    valueText: counter.locator('span').filter({ hasText: /^Value: .*!\s*$/ }).first(),
  };
}

async function expectStaticAssetLoads(request, path, contentTypePattern) {
  const response = await request.get(`${BASE_URL}${path}`);

  expect(response.ok(), `${path} should load successfully`).toBeTruthy();
  expect(response.headers()['content-type']).toMatch(contentTypePattern);
}

test.describe('demo counter', () => {
  test.beforeEach(async ({ page }) => {
    page.setDefaultTimeout(5 * SECOND);
    page.setDefaultNavigationTimeout(5 * SECOND);
    await page.goto(BASE_URL, { waitUntil: 'networkidle' });
  });

  test('increments and decrements to 7', async ({ page }) => {
    const { incrementButton, decrementButton, valueText } = counterControls(page);

    for (let index = 0; index < 10; index += 1) {
      await incrementButton.click();
    }

    for (let index = 0; index < 3; index += 1) {
      await decrementButton.click();
    }

    await expect(valueText).toHaveText('Value: 7!');
  });

  test('increments and clears to 0', async ({ page }) => {
    const { incrementButton, clearButton, valueText } = counterControls(page);

    for (let index = 0; index < 5; index += 1) {
      await incrementButton.click();
    }

    await clearButton.click();

    await expect(valueText).toHaveText('Value: 0!');
  });

  test('loads /static/common.css with the expected mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/common.css', /^text\/css\b/i);
  });

  test('loads /static/common.css and keeps the current mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/common.css', /^text\/css\b/i);
  });

  test('loads /static/bootstrap.js with the current mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/bootstrap.js', /^text\/javascript\b/i);
  });

  test('loads /static/wasm/terrazzo_demo.js with the current mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/wasm/terrazzo_demo.js', /^text\/javascript\b/i);
  });

  test('loads /static/wasm/terrazzo_demo_bg.wasm with the current mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/wasm/terrazzo_demo_bg.wasm', /^application\/wasm\b/i);
  });
});
