import { test, expect } from '@playwright/test';

const SECOND = 1000;
const BASE_URL = process.env.BASE_URL ?? 'http://127.0.0.1:3000';

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
  const failureDetails = `status=${response.status()} headers=${JSON.stringify(response.headers())}`;
  expect(response.ok(), `${path} should load successfully (${failureDetails})`).toBeTruthy();
  expect(response.headers()['content-type']).toMatch(contentTypePattern);
}

async function fontWeightIsBold(locator) {
  const fontWeight = await locator.evaluate((element) => getComputedStyle(element).fontWeight);
  return fontWeight === 'bold' || Number.parseInt(fontWeight, 10) >= 700;
}

async function fontStyleIsItalic(locator) {
  const fontStyle = await locator.evaluate((element) => getComputedStyle(element).fontStyle);
  return fontStyle === 'italic';
}

async function textDecorationHasUnderline(locator) {
  const textDecorationLine = await locator.evaluate(
    (element) => getComputedStyle(element).textDecorationLine,
  );
  return textDecorationLine.includes('underline');
}

async function formatButtonIsActive(button) {
  const className = await button.getAttribute('class');
  return /(?:^|\s)active-[^\s]+/.test(className ?? '');
}

async function setFormatButtonActive(button, active) {
  if ((await formatButtonIsActive(button)) !== active) {
    await button.click();
  }
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

  test('loads /static/bootstrap.js with the current mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/bootstrap.js', /^text\/javascript\b/i);
  });

  test('loads /static/wasm/terrazzo_demo.js with the current mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/wasm/terrazzo_demo.js', /^text\/javascript\b/i);
  });

  test('loads /static/wasm/terrazzo_demo_bg.wasm with the current mime type', async ({ request }) => {
    await expectStaticAssetLoads(request, '/static/wasm/terrazzo_demo_bg.wasm', /^application\/wasm\b/i);
  });

  test('attributes node keeps the bazel class in integration tests', async ({ page }) => {
    await expect(page.locator('#attributes')).toHaveClass(process.env.BAZEL === '1' ? 'bazel' : 'not bazel');
  });

  test('attributes dropdown keeps bold styling in sync with selected flavor', async ({ page }) => {
    const attributes = page.locator('#attributes');
    const flavorSelect = attributes.getByRole('combobox');
    const boldButton = attributes.getByRole('button', { name: 'B', exact: true });
    const italicButton = attributes.getByRole('button', { name: 'I', exact: true });
    const underlineButton = attributes.getByRole('button', { name: 'U', exact: true });
    const result = page.locator('#attributes-result').locator('div').first();
    const flavorNames = await flavorSelect.locator('option').allTextContents();
    const styleChecks = [
      {
        directFlavor: 'BoldD',
        staticFlavor: 'BoldS',
        button: boldButton,
        isApplied: () => fontWeightIsBold(result),
      },
      {
        directFlavor: 'ItalicD',
        staticFlavor: 'ItalicS',
        button: italicButton,
        isApplied: () => fontStyleIsItalic(result),
      },
      {
        directFlavor: 'UnderlineD',
        staticFlavor: 'UnderlineS',
        button: underlineButton,
        isApplied: () => textDecorationHasUnderline(result),
      },
    ];

    for (const flavorName of flavorNames) {
      await flavorSelect.selectOption({ label: flavorName });

      for (const { directFlavor, staticFlavor, button, isApplied } of styleChecks) {
        if (flavorName.includes(staticFlavor)) {
          await setFormatButtonActive(button, false);
          await expect.poll(isApplied).toBe(true);
          await setFormatButtonActive(button, true);
          await expect.poll(isApplied).toBe(true);
          continue;
        }

        if (flavorName.includes(directFlavor)) {
          await setFormatButtonActive(button, false);
          await expect.poll(isApplied).toBe(false);
          await setFormatButtonActive(button, true);
          await expect.poll(isApplied).toBe(true);
        }
      }
    }
  });
});
