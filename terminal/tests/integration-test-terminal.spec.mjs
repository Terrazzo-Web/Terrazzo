import { test, expect } from '@playwright/test';

const SECOND = 1000;
const BASE_URL = (process.env.BASE_URL ?? 'http://127.0.0.1:3000')
  .split(';')
  .map((url) => url.trim())
  .filter(Boolean)[0];

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

function getAddTabButton(page) {
  return page.locator('div.add-tab-icon img');
}

function getTabs(page) {
  return page.locator(
    'div.terminals div.titles > ul > li.title:has(img[class~="close-icon"])',
  );
}

function getActiveTerminal(page) {
  return page.locator(
    'div.terminals div.items > ul > li.selected .xterm',
  );
}

function getCloseIcons(tabs) {
  return tabs.locator('img.close-icon');
}

async function closeTab(tab) {
  await tab.hover();
  await tab.locator('img.close-icon').click();
}

async function clickLocator(locator) {
  await locator.click({ force: true });
}

async function focusTerminal(terminal) {
  await terminal.evaluate((node) => {
    const textarea = node.querySelector('textarea');
    if (textarea) {
      textarea.focus();
    } else {
      node.focus();
    }
  });
}

test.describe('Terminal', () => {
  test.beforeEach(async ({ page }, testInfo) => {
    page.setDefaultTimeout(5 * SECOND);
    page.setDefaultNavigationTimeout(5 * SECOND);
    recordBrowserLogs(page, testInfo);
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
  });

  test.afterEach(async ({}, testInfo) => {
    const browserLogs = testInfo.browserLogs ?? [];
    expect(browserLogs, browserLogs.join('\n\n')).toEqual([]);
  });

  test('loads /static/common.css with the expected mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/common.css', /^text\/css\b/i);
  });

  test('opens a new terminal tab and runs a command', async ({ page }) => {
    const addTabButton = getAddTabButton(page);
    await addTabButton.waitFor({ timeout: 30 * SECOND });
    await expect(addTabButton).toHaveCSS('height', '15px');
    await expect(addTabButton).toHaveCSS('filter', 'invert(1)');
    await addTabButton.click();

    const tabs = getTabs(page);
    const activeTerminal = getActiveTerminal(page);
    await expect(tabs).toHaveCount(1);
    await expect(activeTerminal).toHaveCount(1);

    await focusTerminal(activeTerminal);
    await page.keyboard.type('echo $((191*7))');
    await page.keyboard.press('Enter');
    await expect(activeTerminal).toContainText('1337');

    await closeTab(tabs);
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

    await clickLocator(firstTab);
    await expect(firstTab).toHaveClass(/selected/);
    await expect(page.locator('li.selected .xterm')).toHaveCount(1);

    const activeTerminal = getActiveTerminal(page);
    await focusTerminal(activeTerminal);
    await page.keyboard.type('echo $((191*7))');
    await page.keyboard.press('Enter');
    await expect(activeTerminal).toContainText('1337');

    await clickLocator(secondTab);
    await expect(secondTab).toHaveClass(/selected/);
    await expect(page.locator('li.selected .xterm')).toHaveCount(1);

    await focusTerminal(activeTerminal);
    await page.keyboard.type('echo $((191*7*2))');
    await page.keyboard.press('Enter');
    await expect(activeTerminal).toContainText('2674');
    await expect(activeTerminal).not.toContainText('1337');

    const closeIcons = getCloseIcons(tabs);
    await expect(closeIcons).toHaveCount(2);
    await closeTab(tabs.nth(0));
    await expect(tabs).toHaveCount(1);
    await closeTab(tabs.nth(0));
    await expect(tabs).toHaveCount(0);
  });
});
