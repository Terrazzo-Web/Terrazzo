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

  test('Invalid server_fn endpoint', async ({ page }) => {
    const response = await fetchServerFnFromPage(
      page,
      '/api/fn/invalid_server_fn_endpoint',
      { parameter: "abc" },
    );
    expect(response.status).toBe(400);
    expect(response.body).toContain(
      'Could not find a server function at the route /api/fn/invalid_server_fn_endpoint.',
    );
  });

  test('typing abc shows abc', async ({ page }) => {
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
    await input.pressSequentially('abc');
    const response = await conversionsResponse;
    expect(response.ok()).toBeTruthy();
    expect(response.headers()['content-type']).toMatch(/^application\/octet-stream\b/i);

    await expect(page.locator('pre.converter-output').first()).toHaveText('"abc"');
  });

  test('typing a JWT shows parsed JWT content', async ({ page }) => {
    const jwt = 'eyJhbGciOiJSUzI1NiIsImtpZCI6IjE2In0.eyJpc3MiOiJodHRwczovL29wZW5pZC5leGFtcGxlLmNvbSIsInN1YiI6IjEyMzQ1Njc4OTAiLCJhdWQiOiJjbGllbnQtMTIzIiwiaWF0IjoxNzAwMDAwMDAwLCJleHAiOjE3MDAwMDM2MDAsIm5vbmNlIjoiYWJjMTIzIiwibmFtZSI6IkpvaG4gRG9lIiwiZW1haWwiOiJqb2huQGV4YW1wbGUuY29tIn0.Qh6cZf5tR8wPz7g9m1Xl3k2YV9JpL0aWZx3nF5K8mJp2ZrT7vLw9sX1yQd6fG8hJkL2mN4pQ7rS9tU1vW3xY5zA';

    await page.locator('.app-menu-trigger').hover();
    await page.getByText('Converter', { exact: true }).click();

    const input = page.locator('textarea.converter-input');
    await expect(input).toBeVisible();
    await page.waitForTimeout(500);
    const conversionsResponse = page.waitForResponse((response) =>
      response.request().method() === 'POST' &&
      response.url().includes('/api/fn/get_conversions'),
    );
    await input.fill(jwt);
    const response = await conversionsResponse;
    expect(response.ok()).toBeTruthy();
    expect(response.headers()['content-type']).toMatch(/^application\/octet-stream\b/i);

    const jwtTab = page.getByText('JWT', { exact: true });
    await expect(jwtTab).toBeVisible();
    await jwtTab.click();

    await expect(page.locator('pre.converter-output').first()).toContainText('aud: client-123');
    await expect(page.locator('pre.converter-output').first()).toContainText('email: john@example.com');
    await expect(page.locator('pre.converter-output').first()).toHaveText(
      /exp: 1700003600 = 2023-11-14T23:13:20Z \(.+ ago\)/,
    );
  });

});
