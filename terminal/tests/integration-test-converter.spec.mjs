import { test, expect } from '@playwright/test';

const SECOND = 1000;
const BASE_URLS = (process.env.BASE_URL ?? 'http://127.0.0.1:3000')
  .split(';')
  .map((url) => url.trim())
  .filter(Boolean);
const BASE_URL = BASE_URLS[0];

function recordBrowserLogs(page, testInfo) {
  const browserLogs = [];
  testInfo.browserLogs = browserLogs;

  page.on('console', async (message) => {
    if (message.type() !== 'error') {
      return;
    }

    const values = await Promise.all(message.args().map(async (arg) => {
      try {
        return JSON.stringify(await arg.jsonValue());
      } catch {
        return arg.toString();
      }
    }));
    browserLogs.push([
      `console.${message.type()}: ${message.text()}`,
      ...values.map((value) => `  ${value}`),
      message.location().url ? `  at ${message.location().url}:${message.location().lineNumber}` : '',
    ].filter(Boolean).join('\n'));
  });

  page.on('pageerror', (error) => {
    browserLogs.push(`pageerror: ${error.stack ?? error.message}`);
  });
}

async function expectStaticAssetLoads(request, path, contentTypePattern) {
  const response = await request.get(`${BASE_URL}${path}`);
  const failureDetails = `status=${response.status()} headers=${JSON.stringify(response.headers())}`;
  expect(response.ok(), `${path} should load successfully (${failureDetails})`).toBeTruthy();
  expect(response.headers()['content-type']).toMatch(contentTypePattern);
}

function getConverterInput(page) {
  return page.locator('textarea.converter-input');
}

function getConverterOutput(page) {
  return page.locator('pre.converter-output').first();
}

function waitForConversionsResponse(page) {
  return page.waitForResponse((response) =>
    response.request().method() === 'POST' &&
    response.url().includes('/api/fn/get_conversions'),
  );
}

async function expectConversionsResponse(response) {
  expect(response.ok()).toBeTruthy();
  expect(response.headers()['content-type']).toMatch(/^application\/json\b/i);
}

async function openConverter(page) {
  await page.locator('.app-menu-trigger').hover();
  await page.getByText('Converter', { exact: true }).click();

  const input = getConverterInput(page);
  await expect(input).toBeVisible();
  await page.waitForTimeout(500);
  return input;
}

async function setConverterInput(page, value) {
  const input = getConverterInput(page);
  const conversionsResponse = waitForConversionsResponse(page);
  await input.fill(value);
  await expectConversionsResponse(await conversionsResponse);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

async function showRemoteDropdown(page) {
  const remote = page.locator('.show-remote');
  await remote.hover();

  const options = remote.locator('li');
  await expect(options.filter({ hasText: /^Local$/ })).toBeVisible();
  await expect(options.filter({ hasText: /^test-client/ })).toBeVisible();
  return options;
}

async function selectRemote(page, name) {
  const options = await showRemoteDropdown(page);
  const optionText = name === 'Local'
    ? new RegExp(`^${escapeRegExp(name)}$`)
    : new RegExp(`^${escapeRegExp(name)}`);
  const conversionsResponse = waitForConversionsResponse(page);
  await options.filter({ hasText: optionText }).click();
  await expectConversionsResponse(await conversionsResponse);
}

test.describe('Converter', () => {
  test.beforeEach(async ({ page }, testInfo) => {
    page.setDefaultTimeout(5 * SECOND);
    page.setDefaultNavigationTimeout(5 * SECOND);
    recordBrowserLogs(page, testInfo);
    await page.goto(BASE_URL, { waitUntil: 'networkidle' });
  });

  test.afterEach(async ({}, testInfo) => {
    const browserLogs = testInfo.browserLogs ?? [];
    expect(browserLogs, browserLogs.join('\n\n')).toEqual([]);
  });

  test('loads /static/common.css with the expected mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/common.css', /^text\/css\b/i);
  });

  test('reports two working server endpoints', async ({ request }) => {
    expect(BASE_URLS).toHaveLength(2);

    const ports = BASE_URLS.map((url) => new URL(url).port);
    expect(new Set(ports).size).toBe(2);

    for (const url of BASE_URLS) {
      const response = await request.get(url);
      const failureDetails = `url=${url} status=${response.status()} headers=${JSON.stringify(response.headers())}`;
      expect(response.ok(), `endpoint should respond (${failureDetails})`).toBeTruthy();
    }
  });

  test('Invalid server_fn endpoint', async ({ page }) => {
    const response = await page.request.post(`${BASE_URL}/api/fn/invalid_server_fn_endpoint`, {
      data: { parameter: 'abc' },
    });
    expect(response.status()).toBe(400);
    expect(await response.text()).toContain(
      'Could not find a server function at the route /api/fn/invalid_server_fn_endpoint.',
    );
  });

  test('typing abc shows abc', async ({ page }) => {
    const input = await openConverter(page);
    const conversionsResponse = waitForConversionsResponse(page);
    await input.click();
    await input.pressSequentially('abc');
    await expectConversionsResponse(await conversionsResponse);

    await expect(getConverterOutput(page)).toHaveText('"abc"');
  });

  test('typing a JWT shows parsed JWT content', async ({ page }) => {
    const jwt = 'eyJhbGciOiJSUzI1NiIsImtpZCI6IjE2In0.eyJpc3MiOiJodHRwczovL29wZW5pZC5leGFtcGxlLmNvbSIsInN1YiI6IjEyMzQ1Njc4OTAiLCJhdWQiOiJjbGllbnQtMTIzIiwiaWF0IjoxNzAwMDAwMDAwLCJleHAiOjE3MDAwMDM2MDAsIm5vbmNlIjoiYWJjMTIzIiwibmFtZSI6IkpvaG4gRG9lIiwiZW1haWwiOiJqb2huQGV4YW1wbGUuY29tIn0.Qh6cZf5tR8wPz7g9m1Xl3k2YV9JpL0aWZx3nF5K8mJp2ZrT7vLw9sX1yQd6fG8hJkL2mN4pQ7rS9tU1vW3xY5zA';

    await openConverter(page);
    await setConverterInput(page, jwt);

    const jwtTab = page.getByText('JWT', { exact: true });
    await expect(jwtTab).toBeVisible();
    await jwtTab.click();

    await expect(getConverterOutput(page)).toContainText('aud: client-123');
    await expect(getConverterOutput(page)).toContainText('email: john@example.com');
    await expect(getConverterOutput(page)).toHaveText(
      /exp: 1700003600 = 2023-11-14T23:13:20Z \(.+ ago\)/,
    );
  });

  test('remote selector keeps converter content per remote', async ({ page }) => {
    await openConverter(page);

    if (await getConverterInput(page).inputValue() !== '') {
      await setConverterInput(page, '');
    }

    const helloWorld = { Hello: 'World!' };
    const bonjourMonde = { Bonjour: 'Monde!' };

    await selectRemote(page, 'test-client');
    await setConverterInput(page, JSON.stringify(helloWorld));
    await expect(getConverterOutput(page)).toHaveText(JSON.stringify(helloWorld, null, 2));

    await selectRemote(page, 'Local');
    await expect(page.locator('pre.converter-output')).toHaveCount(0);

    await setConverterInput(page, JSON.stringify(bonjourMonde));
    await expect(getConverterOutput(page)).toHaveText(JSON.stringify(bonjourMonde, null, 2));

    await selectRemote(page, 'test-client');
    await expect(getConverterOutput(page)).toHaveText(JSON.stringify(helloWorld, null, 2));
  });
});
