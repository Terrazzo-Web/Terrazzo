import { test, expect } from '@playwright/test';

const SECOND = 1000;
const BASE_URL = (process.env.BASE_URL ?? 'http://127.0.0.1:3000')
    .split(';')
    .map((url) => url.trim())
    .filter(Boolean)[0];

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
    await tab.locator('img.close-icon').click({ force: true, timeout: SECOND });
}

async function closeAllTabs(page) {
    for (let attempt = 0; attempt < 10 && await getTabs(page).count() > 0; attempt++) {
        await closeTab(getTabs(page).first()).catch(() => { });
        await page.waitForTimeout(100);
    }
}

async function selectTerminalText(page, text) {
    const row = page.locator('.xterm-rows > div')
        .filter({ hasText: text })
        .first();
    await expect(row).toBeVisible();
    const box = await row.boundingBox();
    if (!box) {
        throw new Error(`Could not find terminal text: ${text}`);
    }
    await page.mouse.move(box.x + 1, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width - 1, box.y + box.height / 2, { steps: 20 });
    await page.mouse.up();
}

test.describe('Terminal', () => {
    test.beforeEach(async ({ page }) => {
        page.setDefaultTimeout(5 * SECOND);
        page.setDefaultNavigationTimeout(5 * SECOND);
        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await getAddTabButton(page).waitFor({ timeout: 10 * SECOND });
        await closeAllTabs(page);
    });

    test.afterEach(async ({ page }) => {
        await closeAllTabs(page);
    });

    test('loads /static/common.css with the expected mime type', async ({ request }) => {
        await expectStaticAssetLoads(request, '/static/common.css', /^text\/css\b/i);
    });

    test('loads terminal input overlay icons', async ({ request }) => {
        await expectStaticAssetLoads(request, '/static/icons/paragraph.svg', /^image\/svg\+xml\b/i);
        await expectStaticAssetLoads(request, '/static/icons/mic-fill.svg', /^image\/svg\+xml\b/i);
        await expectStaticAssetLoads(request, '/static/icons/mic-mute-fill.svg', /^image\/svg\+xml\b/i);
        await expectStaticAssetLoads(request, '/static/icons/send-fill.svg', /^image\/svg\+xml\b/i);
    });

    test('opens a new terminal tab and runs a command', async ({ page }) => {
        const addTabButton = getAddTabButton(page);
        await addTabButton.waitFor({ timeout: 10 * SECOND });
        await expect(addTabButton).toHaveCSS('height', '15px');
        await expect(addTabButton).toHaveCSS('filter', 'invert(1)');
        await addTabButton.click();

        const tabs = getTabs(page);
        const activeTerminal = getActiveTerminal(page);
        await expect(tabs).toHaveCount(1);
        await expect(activeTerminal).toHaveCount(1);

        await expect(activeTerminal).toContainText('Welcome to Test Environment');
        await activeTerminal.click();
        await page.keyboard.type('echo $((191*7))');
        await page.keyboard.press('Enter');
        await expect(activeTerminal).toContainText('1337');

        await closeTab(tabs);
        await expect(tabs).toHaveCount(0);
    });

    test('copies selected terminal text and pastes clipboard text', async ({ page, context }) => {
        await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: BASE_URL });

        const addTabButton = getAddTabButton(page);
        await addTabButton.waitFor({ timeout: 10 * SECOND });
        await addTabButton.click();

        const activeTerminal = getActiveTerminal(page);
        await expect(activeTerminal).toContainText('Welcome to Test Environment');
        await activeTerminal.click();
        await selectTerminalText(page, 'Welcome to Test Environment');

        await page.keyboard.press('Control+C');
        await expect.poll(() => page.evaluate(() => navigator.clipboard.readText()))
            .toBe('Welcome to Test Environment');

        await activeTerminal.click();
        await page.keyboard.type('echo ');
        await page.keyboard.press('Control+V');
        await expect(activeTerminal).toContainText('echo Welcome to Test Environment');
        await page.keyboard.press('Enter');
        await expect.poll(() => activeTerminal.evaluate((terminal) => (
            terminal.textContent.match(/Welcome to Test Environment/g) ?? []
        ).length)).toBeGreaterThanOrEqual(3);
    });

    test('sends typed input from the terminal overlay', async ({ page }) => {
        const addTabButton = getAddTabButton(page);
        await addTabButton.waitFor({ timeout: 10 * SECOND });
        await addTabButton.click();

        const activeTerminal = getActiveTerminal(page);
        await expect(activeTerminal).toContainText('Welcome to Test Environment');

        const overlayButton = page.locator('li.selected .input-overlay-button');
        await expect(overlayButton).toBeVisible();
        await expect(overlayButton).toHaveCSS('filter', 'invert(1)');
        await expect(overlayButton).toHaveCSS('opacity', '0.3');
        await overlayButton.click();

        const textarea = page.locator('li.selected .input-overlay-textarea');
        const sendButton = page.locator('li.selected .input-overlay-send');
        await expect(textarea).toBeVisible();
        await expect(sendButton).toHaveCSS('filter', 'invert(1)');
        await expect(sendButton).toHaveCSS('opacity', '0.3');
        await textarea.fill('echo overlay-input-31415\n');
        await expect(sendButton).toHaveCSS('opacity', '1');
        await sendButton.click();

        await expect(activeTerminal).toContainText('overlay-input-31415');
        await expect(textarea).toHaveValue('');
    });

    test('sends mocked speech recognition input from the terminal overlay', async ({ page }) => {
        await page.addInitScript(() => {
            class MockSpeechRecognition {
                constructor() {
                    this.interimResults = false;
                    this.continuous = false;
                    this.lang = 'en-US';
                }

                start() {
                    setTimeout(() => {
                        this.onresult?.({
                            results: [[{ transcript: 'echo speech-overlay-27182\n' }]],
                        });
                    }, 0);
                }

                stop() {
                    this.onend?.();
                }
            }

            Object.defineProperty(window, 'SpeechRecognition', {
                configurable: true,
                value: MockSpeechRecognition,
            });
            Object.defineProperty(window, 'webkitSpeechRecognition', {
                configurable: true,
                value: MockSpeechRecognition,
            });
        });
        await page.reload({ waitUntil: 'domcontentloaded' });
        await getAddTabButton(page).waitFor({ timeout: 10 * SECOND });
        await closeAllTabs(page);

        const addTabButton = getAddTabButton(page);
        await addTabButton.click();

        const activeTerminal = getActiveTerminal(page);
        await expect(activeTerminal).toContainText('Welcome to Test Environment');

        const overlayButton = page.locator('li.selected .input-overlay-button');
        await expect(overlayButton).toBeVisible();
        await overlayButton.click();
        await overlayButton.click();

        const textarea = page.locator('li.selected .input-overlay-textarea');
        const sendButton = page.locator('li.selected .input-overlay-send');
        await expect(textarea).toHaveValue('echo speech-overlay-27182\n');
        await expect(sendButton).toHaveCSS('opacity', '1');
        await sendButton.click();

        await expect(activeTerminal).toContainText('speech-overlay-27182');
        await expect(textarea).toHaveValue('');
    });

    test('two terminals', async ({ page }) => {
        const addTabButton = getAddTabButton(page);
        await addTabButton.waitFor({ timeout: 10 * SECOND });
        await addTabButton.click();
        await addTabButton.click();

        const tabs = getTabs(page);
        await expect(tabs).toHaveCount(2);

        const firstTab = tabs.nth(0);
        const secondTab = tabs.nth(1);

        await firstTab.click();
        await expect(firstTab).toHaveClass(/selected/);
        await expect(page.locator('li.selected .xterm')).toHaveCount(1);

        const activeTerminal = getActiveTerminal(page);
        await activeTerminal.click();
        await page.keyboard.type('echo $((191*7))');
        await page.keyboard.press('Enter');
        await expect(activeTerminal).toContainText('1337');

        await secondTab.click();
        await expect(secondTab).toHaveClass(/selected/);
        await expect(page.locator('li.selected .xterm')).toHaveCount(1);

        await activeTerminal.click();
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
