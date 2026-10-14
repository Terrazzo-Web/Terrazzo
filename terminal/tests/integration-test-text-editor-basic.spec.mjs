import { expect, test } from '@playwright/test';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

import {
    BASE_URL,
    SECOND,
    createCommittedReadme,
    createFolderTree,
    createTempFile,
    editorFindShortcut,
    getCodeMirrorContent,
    getCodeMirrorSearchPanel,
    getFolderFile,
    getMergeViewEditors,
    getSideViewFile,
    getSideViewFolder,
    openFolderFile,
    reopenFolderFile,
    replaceEditorText,
    setBasePath,
} from './text-editor-helpers.mjs';

test.describe('Text editor basic', () => {
    test.describe.configure({ retries: 5 });

    test.beforeEach(async ({ page }) => {
        page.setDefaultTimeout(5 * SECOND);
        page.setDefaultNavigationTimeout(5 * SECOND);
    });

    test('starts the server', async ({ page }) => {
        const response = await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        expect(response?.ok()).toBeTruthy();
    });

    test('edits a file', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const fileName = 'hello.txt';
        const { baseDir, filePath } = await createTempFile(fileName);

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        const editor = getCodeMirrorContent(page);
        await expect(editor).toBeVisible({ timeout: 10 * SECOND });
        await editor.click();
        await page.keyboard.type('Hello, world!');

        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe('Hello, world!');
    });

    test('inserts typed input from the editor overlay', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const fileName = 'overlay.txt';
        const { baseDir, filePath } = await createTempFile(fileName);

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        const editor = getCodeMirrorContent(page);
        await expect(editor).toBeVisible({ timeout: 10 * SECOND });

        const overlayButton = page.locator('.code-mirror-editor .input-overlay-button');
        await expect(overlayButton).toBeVisible();
        await overlayButton.click();

        const textarea = page.locator('.code-mirror-editor .input-overlay-textarea');
        const sendButton = page.locator('.code-mirror-editor .input-overlay-send');
        await expect(textarea).toBeVisible();
        await textarea.fill('Hello from overlay');
        await expect(sendButton).toHaveCSS('opacity', '1');
        await sendButton.click();

        await expect(editor).toContainText('Hello from overlay');
        await expect(textarea).toHaveValue('');
        await expect(textarea).toBeHidden();
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe('Hello from overlay');
    });

    test('inserts mocked speech recognition input from the editor overlay', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        await page.addInitScript(() => {
            window.__speechRecognitionStops = 0;

            class MockSpeechRecognition {
                constructor() {
                    this.interimResults = false;
                    this.continuous = false;
                    this.lang = 'en-US';
                }

                start() {
                    setTimeout(() => {
                        this.onresult?.({
                            results: [[{ transcript: 'Hello from speech overlay' }]],
                        });
                    }, 0);
                }

                stop() {
                    window.__speechRecognitionStops += 1;
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

        const fileName = 'speech-overlay.txt';
        const { baseDir, filePath } = await createTempFile(fileName);

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        const editor = getCodeMirrorContent(page);
        await expect(editor).toBeVisible({ timeout: 10 * SECOND });

        const overlayButton = page.locator('.code-mirror-editor .input-overlay-button');
        await expect(overlayButton).toBeVisible();
        await overlayButton.click();
        await overlayButton.click();

        const textarea = page.locator('.code-mirror-editor .input-overlay-textarea');
        const sendButton = page.locator('.code-mirror-editor .input-overlay-send');
        await expect(textarea).toHaveValue('Hello from speech overlay');
        await expect(sendButton).toHaveCSS('opacity', '1');
        await sendButton.click();

        await expect(editor).toContainText('Hello from speech overlay');
        await expect(textarea).toHaveValue('');
        await expect(textarea).toBeHidden();
        await expect.poll(() => page.evaluate(() => window.__speechRecognitionStops)).toBe(1);
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe('Hello from speech overlay');
    });

    test('remembers the cursor position when reopening a file', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const firstFile = 'first.txt';
        const secondFile = 'second.txt';
        const { baseDir, filePath: firstPath } = await createTempFile(firstFile);
        const secondPath = path.join(baseDir, secondFile);
        await writeFile(firstPath, ['alpha line', 'bravo line', 'charlie line'].join('\n'));
        await writeFile(secondPath, 'Second file');

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, firstFile);
        await openFolderFile(page, firstFile);

        const editor = getCodeMirrorContent(page);
        await expect(editor).toContainText('bravo line', { timeout: 10 * SECOND });
        await editor.click();
        await page.keyboard.press(process.platform === 'darwin' ? 'Meta+ArrowUp' : 'Control+Home');
        await page.keyboard.press('ArrowDown');
        for (let i = 0; i < 'bravo'.length; i += 1) {
            await page.keyboard.press('ArrowRight');
        }
        await page.waitForTimeout(2 * SECOND);

        await getSideViewFolder(page, '').locator('span').first().click();
        await expect(getFolderFile(page, secondFile)).toBeVisible({ timeout: 10 * SECOND });
        await openFolderFile(page, secondFile);

        await expect(getSideViewFile(page, firstFile)).toBeVisible({ timeout: 10 * SECOND });
        await expect(getSideViewFile(page, secondFile)).toBeVisible({ timeout: 10 * SECOND });
        await expect(editor).toContainText('Second file', { timeout: 10 * SECOND });

        await getSideViewFile(page, firstFile).locator('span').first().click();
        await expect(editor).toContainText('bravo line', { timeout: 10 * SECOND });
        await editor.evaluate((node) => node.focus());
        await page.keyboard.insertText(' remembered');
        await expect.poll(async () => readFile(firstPath, 'utf8'), { timeout: 10 * SECOND }).toBe(
            ['alpha line', 'bravo remembered line', 'charlie line'].join('\n'),
        );
        await expect.poll(async () => readFile(secondPath, 'utf8'), { timeout: 10 * SECOND }).toBe('Second file');
    });

    test('finds text in the editor and selects the matching row', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const fileName = 'hello.txt';
        const { baseDir, filePath } = await createTempFile(fileName);
        const content = Array.from({ length: 300 }, (_, index) => `Hello, World! ${index + 1}`).join('\n');

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        const editor = getCodeMirrorContent(page);
        await expect(editor).toBeVisible({ timeout: 10 * SECOND });
        await replaceEditorText(page, editor, content);

        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe(content);

        await page.keyboard.press(editorFindShortcut());

        const searchPanel = getCodeMirrorSearchPanel(page);
        await expect(searchPanel).toBeVisible({ timeout: 10 * SECOND });
        await searchPanel.locator('input[name="search"]').click();
        await page.keyboard.type('! 8');
        await page.keyboard.press('Enter');

        const selectedSearchMatchLine = getCodeMirrorContent(page).locator('.cm-line', {
            has: page.locator('.cm-searchMatch-selected'),
        });
        await expect(selectedSearchMatchLine).toHaveText('Hello, World! 8', { timeout: 10 * SECOND });
    });

    test('shows a git diff for modified files and returns to plain view when reverted', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const { baseDir, fileName, filePath } = await createCommittedReadme('README.txt');

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        await expect(getMergeViewEditors(page)).toHaveCount(0, { timeout: 10 * SECOND });
        const plainEditor = getCodeMirrorContent(page).first();
        await expect(plainEditor).toContainText('Hello, World!', { timeout: 10 * SECOND });

        const diffToggle = page.locator('.toggle-editor-diff');
        await expect(diffToggle).toBeHidden();
        await plainEditor.evaluate((node) => { window.__originalEditor = node; });
        await replaceEditorText(page, plainEditor, 'Bonjour, Monde!');
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe('Bonjour, Monde!');

        // Availability updates without reopening or replacing the live editor.
        await expect.poll(() => plainEditor.evaluate((node) => node === window.__originalEditor)).toBe(true);
        await expect(diffToggle).toBeVisible({ timeout: 10 * SECOND });
        await expect(diffToggle).toHaveAttribute('title', 'Show diff');
        await diffToggle.click();
        await expect(diffToggle).toHaveAttribute('title', 'Hide diff');

        await expect(getMergeViewEditors(page)).toHaveCount(2, { timeout: 10 * SECOND });
        const diffEditors = getCodeMirrorContent(page);
        await expect(diffEditors.nth(0)).toContainText('Hello, World!', { timeout: 10 * SECOND });
        await expect(diffEditors.nth(1)).toContainText('Bonjour, Monde!', { timeout: 10 * SECOND });

        await replaceEditorText(page, diffEditors.nth(1), 'Hello, World!');
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe('Hello, World!');

        await expect(diffToggle).toBeHidden({ timeout: 10 * SECOND });
        await reopenFolderFile(page, fileName);

        await expect(getMergeViewEditors(page)).toHaveCount(0, { timeout: 10 * SECOND });
        await expect(getCodeMirrorContent(page)).toHaveCount(1, { timeout: 10 * SECOND });
        await expect(getCodeMirrorContent(page).first()).toContainText('Hello, World!', { timeout: 10 * SECOND });
    });

    test('updates diff availability while typing in a large file without replacing the editor', async ({ page }) => {
        test.setTimeout(60 * SECOND);
        const original = Array.from({ length: 2000 }, (_, index) => `${index}: ${'unchanged '.repeat(30)}`).join('\n') + '\noriginal';
        const modified = original.slice(0, -8) + 'modified';
        const { baseDir, fileName, filePath } = await createCommittedReadme('large.txt', original);
        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);
        const editor = getCodeMirrorContent(page);
        const diffToggle = page.locator('.toggle-editor-diff');
        await expect(editor).toBeVisible();
        await expect(diffToggle).toBeHidden();
        await editor.evaluate((node) => { window.__largeEditor = node; node.focus(); });
        await page.keyboard.press(process.platform === 'darwin' ? 'Meta+ArrowDown' : 'Control+End');
        for (let index = 0; index < 8; index += 1) await page.keyboard.press('Backspace');
        await page.keyboard.type('modified', { delay: 10 });
        await expect(diffToggle).toBeVisible({ timeout: 10 * SECOND });
        await expect.poll(() => editor.evaluate((node) => node === window.__largeEditor)).toBe(true);
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe(modified);
        await expect(getMergeViewEditors(page)).toHaveCount(0);
    });

    test('toggles unchanged regions with diff controls on the same row', async ({ page }) => {
        test.setTimeout(60 * SECOND);
        const original = Array.from({ length: 60 }, (_, index) => `Original line ${index + 1}`).join('\n');
        const modified = original.replace('Original line 30', 'Modified line 30');
        const { baseDir, fileName, filePath } = await createCommittedReadme('context.txt', original);
        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);
        await replaceEditorText(page, getCodeMirrorContent(page), modified);

        const diffToggle = page.locator('.toggle-editor-diff');
        const contextToggle = page.locator('.toggle-unchanged-regions');
        await expect(diffToggle).toBeVisible();
        await expect(contextToggle).toBeHidden();
        await diffToggle.click();
        await expect(getMergeViewEditors(page)).toHaveCount(2);
        await expect(contextToggle).toHaveAttribute('title', 'Show unchanged regions');
        await expect(contextToggle).toHaveAttribute('src', /chevron-bar-expand\.svg$/);
        const collapsed = page.locator('.code-mirror-editor .cm-collapsedLines');
        await expect(collapsed.first()).toBeVisible();

        // The buttons must be horizontally adjacent, including their actual rendered wrappers.
        await expect.poll(async () => {
            const diff = await diffToggle.boundingBox();
            const context = await contextToggle.boundingBox();
            return diff && context && Math.abs(diff.y - context.y) < 2 && context.x >= diff.x + diff.width;
        }).toBe(true);
        const currentEditor = getCodeMirrorContent(page).nth(1);
        await currentEditor.evaluate((node) => { window.__diffEditor = node; });
        await contextToggle.click();
        await expect(collapsed).toHaveCount(0);
        await expect(contextToggle).toHaveAttribute('title', 'Hide unchanged regions');
        await expect(contextToggle).toHaveAttribute('src', /chevron-bar-contract\.svg$/);
        await expect.poll(() => currentEditor.evaluate((node) =>
            window.JsDeps.EditorView.findFromDOM(node).state.doc.toString(),
        )).toBe(modified);
        await contextToggle.click();
        await expect(collapsed.first()).toBeVisible();
        await expect.poll(() => currentEditor.evaluate((node) => node === window.__diffEditor)).toBe(true);
        await expect.poll(async () => readFile(filePath, 'utf8')).toBe(modified);
        await diffToggle.click();
        await expect(contextToggle).toBeHidden();
        await expect(getMergeViewEditors(page)).toHaveCount(0);
        await expect(getCodeMirrorContent(page)).toContainText('Modified line 30');
    });

    test('throttles full-document callbacks during continuous typing and delivers the final edit', async ({ page }) => {
        test.setTimeout(60 * SECOND);
        const { baseDir, filePath } = await createTempFile('throttled.txt');
        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, 'throttled.txt');
        await openFolderFile(page, 'throttled.txt');
        const editor = getCodeMirrorContent(page);
        await expect(editor).toBeVisible();
        await page.evaluate(async () => {
            const url = performance.getEntriesByType('resource').find(entry => entry.name.endsWith('/code_mirror.js')).name;
            const { ChangeNotifier } = await import(url);
            const flush = ChangeNotifier.prototype.flush;
            window.__changeTimes = [];
            ChangeNotifier.prototype.flush = function () {
                if (this.pending !== null) window.__changeTimes.push(performance.now());
                return flush.call(this);
            };
        });
        await editor.click();
        const content = 'continuous typing test';
        await page.keyboard.type(content, { delay: 120 });
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe(content);
        const times = await page.evaluate(() => window.__changeTimes);
        expect(times.length).toBeGreaterThanOrEqual(3);
        expect(times.length).toBeLessThanOrEqual(5);
        for (let index = 1; index < times.length; index++) {
            expect(times[index] - times[index - 1]).toBeGreaterThanOrEqual(999);
        }
    });

    test('flushes pending edits before rebuilding the diff editor', async ({ page }) => {
        test.setTimeout(60 * SECOND);
        const { baseDir, fileName, filePath } = await createCommittedReadme('pending.txt');
        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);
        const editor = getCodeMirrorContent(page);
        await replaceEditorText(page, editor, 'First edit');
        const diff = page.locator('.toggle-editor-diff');
        await expect(diff).toBeVisible();
        await editor.evaluate(node => {
            const view = window.JsDeps.EditorView.findFromDOM(node);
            view.dispatch({ changes: { from: view.state.doc.length, insert: ' with pending text' } });
        });
        await diff.click();
        await expect(getMergeViewEditors(page)).toHaveCount(2);
        await expect(getCodeMirrorContent(page).nth(1)).toHaveText('First edit with pending text');
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe('First edit with pending text');
    });

    test('expands and collapses side-view folder nodes', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const root = await createFolderTree();

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, root, 'a');
        await openFolderFile(page, 'a/');
        await openFolderFile(page, 'a2.txt');

        await expect(getCodeMirrorContent(page)).toContainText('I am Bob', { timeout: 10 * SECOND });
        await expect(getSideViewFile(page, 'a/a2.txt')).toBeVisible({ timeout: 10 * SECOND });
        await expect(getSideViewFile(page, 'a/a1.txt')).toHaveCount(0);

        const folderA = getSideViewFolder(page, 'a');
        await expect(folderA).toBeVisible({ timeout: 10 * SECOND });
        await folderA.hover();
        await folderA.locator('.side-view-expand-folder').click();

        await expect(getSideViewFile(page, 'a/a1.txt')).toBeVisible({ timeout: 10 * SECOND });
        await expect(getSideViewFolder(page, 'a/c')).toBeVisible({ timeout: 10 * SECOND });
        await expect(getSideViewFile(page, 'a/c/c.txt')).toHaveCount(0);

        await folderA.locator('.side-view-collapse-folder').click();

        await expect(getSideViewFile(page, 'a/a1.txt')).toHaveCount(0, { timeout: 10 * SECOND });
        await expect(getSideViewFolder(page, 'a/c')).toHaveCount(0, { timeout: 10 * SECOND });
    });
});
