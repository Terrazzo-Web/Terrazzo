import { test, expect } from '@playwright/test';

const SECOND = 1000;
const BASE_URLS = (process.env.BASE_URL ?? 'http://127.0.0.1:3000')
    .split(';')
    .map((url) => url.trim())
    .filter(Boolean);
const BASE_URL = BASE_URLS[0];

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

function getConverterInput(page) {
    return page.locator('textarea.converter-input');
}

function getConverterOutput(page) {
    return page.locator('pre.converter-output').first();
}

function getConverterResizeBar(page) {
    return page.locator('textarea.converter-input + .resize-bar-horz').first();
}

function getTileResizeBars(page) {
    return page.locator('.tile-array > .resize-bar-horz');
}

function getTiles(page) {
    return page.locator('.tile-array > .app-tile');
}

function getResizeBarGrip(resizeBar) {
    return resizeBar.locator(':scope > div').first();
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
    await page.locator('.app-menu-trigger').first().hover();
    await page.getByText('Converter', { exact: true }).click();

    const input = getConverterInput(page);
    await expect(input).toBeVisible();
    await page.waitForTimeout(500);
    return input;
}

async function clickVerticalSplitter(page) {
    await page.locator('.app-menu-trigger').first().hover();
    const button = page.locator('img.split-horizontal').first();
    await button.waitFor({ state: 'visible' });
    await button.click();
}

async function clickTabbedSplitter(page) {
    await page.locator('.app-menu-trigger').first().hover();
    const button = page.locator('img.split-tabbed').first();
    await button.waitFor({ state: 'visible' });
    await button.click();
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

async function getResizeBarStyles(resizeBar) {
    return resizeBar.evaluate((bar) => {
        const grip = bar.firstElementChild;
        const line = grip?.firstElementChild;
        const gripStyle = window.getComputedStyle(grip);
        const lineStyle = window.getComputedStyle(line);
        return {
            barPosition: window.getComputedStyle(bar).position,
            barWidth: window.getComputedStyle(bar).width,
            cursor: gripStyle.cursor,
            paddingLeft: gripStyle.paddingLeft,
            paddingRight: gripStyle.paddingRight,
            linePosition: lineStyle.position,
            lineTransitionDuration: lineStyle.transitionDuration,
        };
    });
}

async function getBoundingBox(locator) {
    const box = await locator.boundingBox();
    expect(box).not.toBeNull();
    return box;
}

test.describe('Converter', () => {
    test.beforeEach(async ({ page }) => {
        page.setDefaultTimeout(5 * SECOND);
        page.setDefaultNavigationTimeout(5 * SECOND);
        await page.route('**/*', (route) => route.continue());
        await page.goto(BASE_URL, { waitUntil: 'networkidle' });
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

    test('vertical tile splitter reuses converter separator styling', async ({ page }) => {
        await openConverter(page);

        const converterResizeBar = getConverterResizeBar(page);
        await expect(converterResizeBar).toBeAttached();
        const converterResizeBarStyles = await getResizeBarStyles(converterResizeBar);

        await clickVerticalSplitter(page);

        const tiles = getTiles(page);
        const tileResizeBars = getTileResizeBars(page);
        const tileResizeBar = tileResizeBars.first();
        await expect(tiles).toHaveCount(2);
        await expect(tileResizeBar).toBeAttached();
        await expect(tileResizeBars).toHaveCount(1);
        await expect(getConverterInput(page)).toHaveCount(1);

        await expect(
            await getResizeBarStyles(tileResizeBar),
        ).toEqual(converterResizeBarStyles);
    });

    test('tabbed tile splitter creates a tab strip and adds tile tabs', async ({ page }) => {
        await openConverter(page);

        await clickTabbedSplitter(page);

        const tabbedTile = page.locator('.tabbed-tile');
        const tabTitles = tabbedTile.locator('.tile-tab-title');
        const tabItems = tabbedTile.locator('.tile-tab-item');
        await expect(tabbedTile).toBeVisible();
        await expect(tabTitles).toHaveCount(2);
        await expect(tabItems).toHaveCount(2);
        await expect(tabTitles.first()).toContainText('New tile');
        await expect(tabItems.first().locator('.app-menu-trigger')).toBeAttached();
        await expect(tabbedTile.locator('.converter-input')).toBeAttached();

        const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
        await tabTitles.first().dispatchEvent('dragstart', { dataTransfer });
        const visibleSeparators = tabbedTile.locator('li[class*="title-show-sep"]');
        await expect(visibleSeparators).toHaveCount(3);
        await expect(visibleSeparators.first()).toBeVisible();
        await tabTitles.first().dispatchEvent('dragend', { dataTransfer });
        await dataTransfer.dispose();

        await tabbedTile.locator('.add-tile-tab > div').click();

        await expect(tabTitles).toHaveCount(3);
        await expect(tabItems).toHaveCount(3);
        await expect(tabbedTile.locator('.tile-tab-title.selected')).toHaveCount(1);
        await expect(tabbedTile.locator('.tile-tab-item.selected .app-menu-trigger')).toBeVisible();
    });
});
