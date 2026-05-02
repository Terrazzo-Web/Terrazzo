import { expect, test } from '@playwright/test';

const SECOND = 1000;
const BASE_URL = process.env.BASE_URL ?? 'http://127.0.0.1:3000';

test.describe('Text editor', () => {
  test.beforeEach(async ({ page }) => {
    page.setDefaultTimeout(5 * SECOND);
    page.setDefaultNavigationTimeout(5 * SECOND);
  });

  test('starts the server', async ({ page }) => {
    const response = await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    expect(response?.ok()).toBeTruthy();
  });
});
