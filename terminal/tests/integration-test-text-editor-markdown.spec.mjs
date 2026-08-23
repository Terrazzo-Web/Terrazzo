import { expect, test } from '@playwright/test';
import { readFile, writeFile } from 'node:fs/promises';

import {
    BASE_URL,
    SECOND,
    createCommittedReadme,
    createTempFile,
    getCodeMirrorContent,
    getHtmlViewerFrame,
    getMilkdownContent,
    getMilkdownMergeViewEditors,
    getMilkdownSource,
    getMilkdownWysiwyg,
    openFolderFile,
    reopenFolderFile,
    replaceEditorText,
    setBasePath,
} from './text-editor-helpers.mjs';

test.describe('Markdown editor', () => {
    test.describe.configure({ retries: 5 });

    test.beforeEach(async ({ page }) => {
        page.setDefaultTimeout(5 * SECOND);
        page.setDefaultNavigationTimeout(5 * SECOND);
    });

    test('synchronizes Milkdown, Markdown source, and disk', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const fileName = 'notes.md';
        const { baseDir, filePath } = await createTempFile(fileName);
        const initial = [
            '# Milkdown heading',
            '',
            'Initial *Markdown* paragraph.',
            '',
            '[Milkdown](https://milkdown.dev/)',
            '',
            '- first item',
            '- second item',
            '',
            '```js',
            'console.log("milkdown");',
            '```',
        ].join('\n');
        await writeFile(filePath, initial);

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        const wysiwyg = getMilkdownWysiwyg(page);
        const milkdownContent = getMilkdownContent(page);
        const source = getMilkdownSource(page);
        const previewToggle = page.locator('.toggle-html-preview');
        await expect(wysiwyg).toBeVisible({ timeout: 10 * SECOND });
        await expect(wysiwyg).toHaveAttribute('data-milkdown-ready', 'true', {
            timeout: 10 * SECOND,
        });
        await expect(source).not.toBeVisible();
        await expect(previewToggle).toHaveAttribute('title', 'Show editor');
        await expect(milkdownContent.locator('h1')).toHaveText('Milkdown heading');
        await expect(milkdownContent.locator('em')).toHaveText('Markdown');
        await expect(milkdownContent.locator('a')).toHaveText('Milkdown');
        await expect(milkdownContent.locator('li')).toHaveCount(2);
        await expect(milkdownContent.locator('.milkdown-code-block')).toContainText('console.log');
        await expect(source).toContainText('# Milkdown heading');

        await page.locator('.refresh-editor').click();
        await expect(page.locator('.toggle-html-preview')).toBeVisible({ timeout: 10 * SECOND });
        await expect(getMilkdownWysiwyg(page)).toHaveAttribute('data-milkdown-ready', 'true', {
            timeout: 10 * SECOND,
        });

        const paragraph = milkdownContent.locator('p').first();
        await paragraph.click();
        await page.keyboard.press('End');
        await page.keyboard.insertText(' Edited in Milkdown.');

        await expect(paragraph).toContainText('Edited in Milkdown.');
        await expect(source).toContainText('Edited in Milkdown.', { timeout: 10 * SECOND });
        await expect
            .poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND })
            .toContain('Edited in Milkdown.');

        await previewToggle.click();
        await expect(wysiwyg).not.toBeVisible();
        await expect(source).toBeVisible();
        await expect(previewToggle).toHaveAttribute('title', 'Show preview and editor');

        const sourceReplacement = '# Source heading\n\nUpdated from **CodeMirror**.\n';
        await replaceEditorText(page, source, sourceReplacement);
        await expect(milkdownContent.locator('h1')).toHaveText('Source heading', { timeout: 10 * SECOND });
        await expect(milkdownContent.locator('strong')).toHaveText('CodeMirror');
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe(sourceReplacement);

        await previewToggle.click();
        await expect(wysiwyg).toBeVisible();
        await expect(source).toBeVisible();
        await expect(previewToggle).toHaveAttribute('title', 'Show preview');

        const externalReplacement = '# External heading\n\nUpdated directly on disk.';
        await writeFile(filePath, externalReplacement);
        await expect(source).toContainText('# External heading', { timeout: 10 * SECOND });
        await expect(milkdownContent.locator('h1')).toHaveText('External heading', { timeout: 10 * SECOND });
        await page.waitForTimeout(2 * SECOND);
        await expect.poll(async () => readFile(filePath, 'utf8')).toBe(externalReplacement);
    });

    test('keeps the Milkdown pane while showing a source diff', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const { baseDir, fileName, filePath } = await createCommittedReadme();

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        const previewToggle = page.locator('.toggle-html-preview');
        await previewToggle.click();
        const source = getMilkdownSource(page);
        await expect(source).toContainText('Hello, World!', { timeout: 10 * SECOND });
        await replaceEditorText(page, source, '# Working copy\n\nChanged in Markdown.');
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toContain('Working copy');

        await reopenFolderFile(page, fileName);
        await previewToggle.click();
        const diffToggle = page.locator('.toggle-editor-diff');
        await expect(diffToggle).toBeVisible({ timeout: 10 * SECOND });
        await diffToggle.click();

        await expect(getMilkdownWysiwyg(page)).toBeVisible({ timeout: 10 * SECOND });
        const diffEditors = getMilkdownMergeViewEditors(page);
        await expect(diffEditors).toHaveCount(2, { timeout: 10 * SECOND });
        const diffContents = diffEditors.locator('.cm-content');
        await expect(diffContents.nth(0)).toContainText('Hello, World!');
        await expect(diffContents.nth(1)).toContainText('Working copy');

        await replaceEditorText(page, diffContents.nth(1), '# Diff edit\n\nSaved from the working side.');
        await expect(getMilkdownContent(page).locator('h1')).toHaveText('Diff edit', { timeout: 10 * SECOND });
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toContain('Diff edit');
    });

    test('keeps CodeMirror source authoritative when Milkdown normalizes Markdown', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const fileName = 'slides.md';
        const { baseDir, filePath } = await createTempFile(fileName);
        const initial = [
            '---',
            'theme: light-icons',
            'layout: two-cols',
            '---',
            '',
            '# Slide title',
            '',
            '::right::',
            '',
            '![Meaningful alt text](./image.png)',
        ].join('\n');
        await writeFile(filePath, initial);

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        await expect(getMilkdownWysiwyg(page)).toBeVisible({ timeout: 10 * SECOND });
        await page.waitForTimeout(2 * SECOND);
        await expect.poll(async () => readFile(filePath, 'utf8')).toBe(initial);

        const previewToggle = page.locator('.toggle-html-preview');
        await previewToggle.click();
        const source = getMilkdownSource(page);
        await expect(source).toBeVisible();
        const replacement = initial.replace('Slide title', 'Source-owned title');
        await replaceEditorText(page, source, replacement);
        await expect
            .poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND })
            .toBe(replacement);
        await page.waitForTimeout(2 * SECOND);
        await expect.poll(async () => readFile(filePath, 'utf8')).toBe(replacement);
    });

    test('does not focus or scroll Milkdown when hovered', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const fileName = 'long-notes.md';
        const { baseDir, filePath } = await createTempFile(fileName);
        const paragraphs = Array.from(
            { length: 80 },
            (_, index) => `Paragraph ${index + 1}: enough text to make the Milkdown pane scroll.`,
        );
        await writeFile(filePath, paragraphs.join('\n\n'));

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        const wysiwyg = getMilkdownWysiwyg(page);
        const content = getMilkdownContent(page);
        await expect(wysiwyg).toHaveAttribute('data-milkdown-ready', 'true', {
            timeout: 10 * SECOND,
        });

        await content.locator('p').last().click();
        const previewToggle = page.locator('.toggle-html-preview');
        await previewToggle.focus();
        await wysiwyg.evaluate((pane) => {
            pane.scrollTop = 0;
        });

        await previewToggle.hover();
        await wysiwyg.hover();
        await page.waitForTimeout(500);

        await expect.poll(() => wysiwyg.evaluate((pane) => pane.scrollTop)).toBe(0);
        await expect(previewToggle).toBeFocused();
    });

    test('keeps existing text and HTML routing', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const textFileName = 'notes.txt';
        const textFixture = await createTempFile(textFileName);
        await writeFile(textFixture.filePath, 'Plain text stays in CodeMirror.');

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, textFixture.baseDir, textFileName);
        await openFolderFile(page, textFileName);
        await expect(getCodeMirrorContent(page)).toContainText('Plain text stays in CodeMirror.', {
            timeout: 10 * SECOND,
        });
        await expect(getMilkdownWysiwyg(page)).toHaveCount(0);

        const htmlFileName = 'preview.html';
        const htmlFixture = await createTempFile(htmlFileName);
        await writeFile(htmlFixture.filePath, '<h1>HTML preview remains available.</h1>');
        await setBasePath(page, htmlFixture.baseDir, htmlFileName);
        await openFolderFile(page, htmlFileName);
        await expect(getHtmlViewerFrame(page)).toBeVisible({ timeout: 10 * SECOND });
        await expect(page.locator('.toggle-html-preview')).toBeVisible();
    });
});
