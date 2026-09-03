import { test, expect } from '@playwright/test';

const SECOND = 1000;
const BASE_URL = (process.env.BASE_URL ?? 'http://127.0.0.1:3000')
    .split(';')
    .map((url) => url.trim())
    .filter(Boolean)[0];

function getConverterInput(page) {
    return page.locator('textarea.converter-input');
}

function getConverterOutput(page) {
    return page.locator('pre.converter-output').first();
}

async function expectConverterOutput(page, expected) {
    await expect.poll(async () =>
        JSON.parse(await getConverterOutput(page).textContent()),
    ).toEqual(expected);
}

function waitForConversionsResponse(page) {
    return page.waitForResponse((response) =>
        response.request().method() === 'POST' &&
        response.url().includes('/api/fn/get_conversions'),
    );
}

async function openConverter(page) {
    await page.locator('.app-menu-trigger').first().hover();
    await page.getByText('Converter', { exact: true }).click();
    const input = getConverterInput(page);
    await expect(input).toBeVisible();
    return input;
}

async function setConverterInput(page, value) {
    const response = waitForConversionsResponse(page);
    await getConverterInput(page).fill(value);
    expect((await response).ok()).toBeTruthy();
}

async function selectRemote(page, name) {
    const remote = page.locator('.show-remote');
    await remote.hover();
    const option = remote.locator('li').filter({
        hasText: name === 'Local' ? /^Local$/ : new RegExp(`^${name}`),
    });
    await expect(option).toBeVisible({ timeout: 20 * SECOND });
    const response = waitForConversionsResponse(page);
    await option.click();
    expect((await response).ok()).toBeTruthy();
}

test('browser calls a terminal client through its WebRTC mesh tunnel', async ({ page }) => {
    test.setTimeout(60 * SECOND);
    page.setDefaultTimeout(10 * SECOND);
    await page.goto(BASE_URL, { waitUntil: 'networkidle' });
    await openConverter(page);

    const remoteValue = { Transport: 'WebRTC', Marker: 'p2p-271828' };
    const localValue = { Transport: 'Local', Marker: 'local-314159' };

    await selectRemote(page, 'test-client');
    await setConverterInput(page, JSON.stringify(remoteValue));
    await expectConverterOutput(page, remoteValue);

    await selectRemote(page, 'Local');
    await setConverterInput(page, JSON.stringify(localValue));
    await expectConverterOutput(page, localValue);

    await selectRemote(page, 'test-client');
    await expectConverterOutput(page, remoteValue);
});
