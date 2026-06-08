import { expect, test } from '@playwright/test';
import { readFile } from 'node:fs/promises';

import {
    BASE_URL,
    SECOND,
    createCommittedReadme,
    createFolderTree,
    createTempFile,
    editorFindShortcut,
    getCodeMirrorContent,
    getCodeMirrorSearchPanel,
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

        const { baseDir, fileName, filePath } = await createCommittedReadme();

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        await expect(getMergeViewEditors(page)).toHaveCount(0, { timeout: 10 * SECOND });
        const plainEditor = getCodeMirrorContent(page).first();
        await expect(plainEditor).toContainText('Hello, World!', { timeout: 10 * SECOND });

        await replaceEditorText(page, plainEditor, 'Bonjour, Monde!');
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe('Bonjour, Monde!');

        await reopenFolderFile(page, fileName);

        const diffToggle = page.locator('.toggle-editor-diff');
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

        await reopenFolderFile(page, fileName);

        await expect(getMergeViewEditors(page)).toHaveCount(0, { timeout: 10 * SECOND });
        await expect(getCodeMirrorContent(page)).toHaveCount(1, { timeout: 10 * SECOND });
        await expect(getCodeMirrorContent(page).first()).toContainText('Hello, World!', { timeout: 10 * SECOND });
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
