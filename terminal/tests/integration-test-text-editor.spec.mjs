import { expect, test } from '@playwright/test';
import { execFile } from 'node:child_process';
import { copyFile, mkdir, mkdtemp, readFile, readdir, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';

const SECOND = 1000;
const execFileAsync = promisify(execFile);
const BASE_URL = (process.env.BASE_URL ?? 'http://127.0.0.1:3000')
    .split(';')
    .map((url) => url.trim())
    .filter(Boolean)[0];
const WORKSPACE_ROOT = path.join(process.env.TEST_SRCDIR ?? '.', process.env.TEST_WORKSPACE ?? '.');
const PLANTUML_PDF = path.join(WORKSPACE_ROOT, 'terminal/tests/PlantUML.pdf');

async function createTempFile(name) {
    const baseDir = await mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'text-editor-'));
    const filePath = path.join(baseDir, name);
    await writeFile(filePath, '');
    return { baseDir, filePath };
}

function getBasePathInput(page) {
    return page.locator('.base-path-selector-field');
}

function getBasePathDisplay(page) {
    return page.locator('.base-path-selector-display');
}

function getCodeMirrorContent(page) {
    return page.locator('.code-mirror-editor .cm-content');
}

function getCodeMirrorSearchPanel(page) {
    return page.locator('.code-mirror-editor .cm-search');
}

function getMergeViewEditors(page) {
    return page.locator('.code-mirror-editor .cm-mergeViewEditor');
}

function editorFindShortcut() {
    return process.platform === 'darwin' ? 'Meta+F' : 'Control+F';
}

function getSideViewFile(page, filePath) {
    return page.locator(`.side-view [data-file-path="${filePath}"]`);
}

function getSideViewFolder(page, folderPath) {
    return page.locator(`.side-view [data-folder-path="${folderPath}"] .side-view-folder-row`);
}

function getFolderFile(page, name) {
    return page.locator('.folder-row', { has: page.locator('.folder-name', { hasText: name }) });
}

function getFolderTrashIcon(page, name) {
    return getFolderFile(page, name).locator('.folder-trash-icon');
}

function getPdfPage(page, pageNumber) {
    return page.locator(`.pdf-viewer canvas[data-page-number="${pageNumber}"]`);
}

function getPdfTextLayer(page, pageNumber) {
    return page.locator(`.pdf-viewer [data-layer="pages"] > div[data-page-number="${pageNumber}"] [data-layer="text"]`);
}

function getPdfAnnotationLayer(page, pageNumber) {
    return page.locator(
        `.pdf-viewer [data-layer="pages"] > div[data-page-number="${pageNumber}"] [data-layer="annotations"]`,
    );
}

function getPdfZoomSlider(page) {
    return page.locator('.pdf-viewer [data-control="zoom"] input[type="range"]');
}

function getPdfZoomValue(page) {
    return page.locator('.pdf-viewer [data-control="zoom"] output');
}

function getCreateFileIcon(page) {
    return page.locator('.create-file-icon');
}

function getCreateFolderIcon(page) {
    return page.locator('.create-folder-icon');
}

function getCreateEntryField(page) {
    return page.locator('.create-entry-field');
}

async function selectPdfZoom(page, percent) {
    await getPdfZoomSlider(page).evaluate((node, percent) => {
        node.value = `${Math.log10(percent / 100)}`;
        node.dispatchEvent(new Event('input', { bubbles: true }));
    }, percent);
}

async function expectPdfPage(page, pageNumber) {
    const viewer = page.locator('.pdf-viewer');
    await expect(viewer).toBeVisible({ timeout: 30 * SECOND });

    const canvas = getPdfPage(page, pageNumber);
    try {
        await expect(canvas).toBeVisible({ timeout: 30 * SECOND });
    } catch (error) {
        const status = await viewer.locator('.pdf-status').textContent().catch(() => '<none>');
        throw new Error(`PDF page ${pageNumber} did not render. Status: ${status}\n${error.message}`);
    }
    return canvas;
}

async function selectedPdfText(page, pageNumber) {
    return page.evaluate((pageNumber) => {
        const textLayer = document.querySelector(
            `.pdf-viewer [data-layer="pages"] > div[data-page-number="${pageNumber}"] [data-layer="text"]`,
        );
        if (!textLayer) {
            return '';
        }

        const range = document.createRange();
        range.selectNodeContents(textLayer);
        const selection = getSelection();
        selection.removeAllRanges();
        selection.addRange(range);
        return selection.toString();
    }, pageNumber);
}

async function renderedCssWidth(canvas) {
    return canvas.evaluate((node) => node.getBoundingClientRect().width);
}

async function renderedPixelCount(canvas) {
    return canvas.evaluate((node) => {
        const context = node.getContext('2d');
        const { width, height } = node;
        if (!context || width === 0 || height === 0) {
            return { width, height, paintedPixels: 0 };
        }

        const data = context.getImageData(0, 0, width, height).data;
        let paintedPixels = 0;
        const sampleStride = 64;
        for (let i = 0; i < data.length; i += 4 * sampleStride) {
            const red = data[i];
            const green = data[i + 1];
            const blue = data[i + 2];
            const alpha = data[i + 3];
            if (alpha !== 0 && (red !== 255 || green !== 255 || blue !== 255)) {
                paintedPixels += 1;
            }
        }
        return { width, height, paintedPixels };
    });
}

async function showBasePathInput(page, timeout = 30 * SECOND) {
    const basePathInput = getBasePathInput(page);
    await page
        .locator('.base-path-selector-field, .base-path-selector-display')
        .first()
        .waitFor({ state: 'visible', timeout });
    if (!(await basePathInput.isVisible().catch(() => false))) {
        await getBasePathDisplay(page).dblclick({ timeout });
    }
    await expect(basePathInput).toBeVisible({ timeout });
    return basePathInput;
}

async function setBasePath(page, baseDir, expectedFileName, timeout = 90 * SECOND) {
    await expect
        .poll(
            async () => {
                try {
                    const basePathInput = await showBasePathInput(page, SECOND);
                    await basePathInput.fill(baseDir);
                    await page.keyboard.press('Tab');
                    await getFolderFile(page, expectedFileName).waitFor({
                        state: 'visible',
                        timeout: SECOND,
                    });
                    return true;
                } catch {
                    return false;
                }
            },
            { timeout },
        )
        .toBe(true);
}

async function openFolderFile(page, name, timeout = 60 * SECOND) {
    await expect
        .poll(
            async () => {
                try {
                    await getFolderFile(page, name).evaluate((node) => node.click(), {
                        timeout: SECOND,
                    });
                    return true;
                } catch {
                    return false;
                }
            },
            { timeout },
        )
        .toBe(true);
}

async function reloadFolder(page, baseDir, fileName) {
    await setBasePath(page, baseDir, fileName);
    await openFolderFile(page, fileName);
}

async function git(cwd, args) {
    await execFileAsync('git', args, { cwd });
}

async function createCommittedReadme() {
    const baseDir = await mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'text-editor-git-'));
    const fileName = 'README.md';
    const filePath = path.join(baseDir, fileName);
    await git(baseDir, ['init']);
    await git(baseDir, ['config', 'user.email', 'test@example.com']);
    await git(baseDir, ['config', 'user.name', 'Test User']);
    await writeFile(filePath, 'Hello, World!');
    await git(baseDir, ['add', fileName]);
    await git(baseDir, ['commit', '-m', 'Add README']);
    return { baseDir, fileName, filePath };
}

async function createFolderTree() {
    const root = await mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'text-editor-tree-'));
    await mkdir(path.join(root, 'a', 'c'), { recursive: true });
    await mkdir(path.join(root, 'b'), { recursive: true });
    await writeFile(path.join(root, 'a', 'a1.txt'), 'I am Alice');
    await writeFile(path.join(root, 'a', 'a2.txt'), 'I am Bob');
    await writeFile(path.join(root, 'a', 'c', 'c.txt'), 'I am Charlie');
    return root;
}

async function exists(filePath) {
    return stat(filePath)
        .then(() => true)
        .catch(() => false);
}

async function isDirectory(filePath) {
    return stat(filePath)
        .then((metadata) => metadata.isDirectory())
        .catch(() => false);
}

async function readFileOrMissing(filePath) {
    return readFile(filePath, 'utf8').catch(() => '<missing>');
}

async function trashEntriesWithContent(trashDir, stem) {
    const entries = await readdir(trashDir).catch(() => []);
    const matching = [];
    for (const entry of entries.filter((entry) => entry.startsWith(stem))) {
        matching.push([entry, await readFileOrMissing(path.join(trashDir, entry))]);
    }
    return matching.sort(([a], [b]) => a.localeCompare(b));
}

function todayUtc() {
    return new Date().toISOString().slice(0, 10);
}

function integrationTrashDir() {
    return path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'terrazzo-integration-test', 'trash');
}

async function replaceEditorText(page, editor, content) {
    await editor.click();
    await page.keyboard.press(process.platform === 'darwin' ? 'Meta+A' : 'Control+A');
    await page.keyboard.insertText(content);
}

test.describe('Text editor', () => {
    test.beforeEach(async ({ page }) => {
        page.setDefaultTimeout(5 * SECOND);
        page.setDefaultNavigationTimeout(5 * SECOND);
    });

    test('starts the server', async ({ page }) => {
        const response = await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        expect(response?.ok()).toBeTruthy();
    });

    test('edits a file', async ({ page }) => {
        test.setTimeout(120 * SECOND);

        const fileName = 'hello.txt';
        const { baseDir, filePath } = await createTempFile(fileName);

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });

        await setBasePath(page, baseDir, fileName);

        await openFolderFile(page, fileName);

        const editor = getCodeMirrorContent(page);
        await expect(editor).toBeVisible({ timeout: 30 * SECOND });
        await editor.click();
        await page.keyboard.type('Hello, world!');

        await expect
            .poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND })
            .toBe('Hello, world!');
    });

    test('finds text in the editor and selects the matching row', async ({ page }) => {
        test.setTimeout(120 * SECOND);

        const fileName = 'hello.txt';
        const { baseDir, filePath } = await createTempFile(fileName);
        const content = Array.from({ length: 300 }, (_, index) => `Hello, World! ${index + 1}`).join('\n');

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });

        await setBasePath(page, baseDir, fileName);

        await openFolderFile(page, fileName);

        const editor = getCodeMirrorContent(page);
        await expect(editor).toBeVisible({ timeout: 30 * SECOND });
        await replaceEditorText(page, editor, content);

        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe(content);

        await page.keyboard.press(editorFindShortcut());

        const searchPanel = getCodeMirrorSearchPanel(page);
        await expect(searchPanel).toBeVisible({ timeout: 30 * SECOND });
        await searchPanel.locator('input[name="search"]').click();
        await page.keyboard.type('! 8');
        await page.keyboard.press('Enter');

        const selectedSearchMatchLine = getCodeMirrorContent(page).locator('.cm-line', {
            has: page.locator('.cm-searchMatch-selected'),
        });
        await expect(selectedSearchMatchLine).toHaveText('Hello, World! 8', { timeout: 10 * SECOND });
    });

    test('renders a PDF file', async ({ page }) => {
        test.setTimeout(120 * SECOND);

        const baseDir = await mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'text-editor-pdf-'));
        await copyFile(PLANTUML_PDF, path.join(baseDir, 'PlantUML.pdf'));

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });

        await setBasePath(page, baseDir, 'PlantUML.pdf');

        await openFolderFile(page, 'PlantUML.pdf');

        await expect(getSideViewFile(page, 'PlantUML.pdf')).toBeVisible({ timeout: 30 * SECOND });

        const firstPage = await expectPdfPage(page, 1);
        await expect
            .poll(async () => (await renderedPixelCount(firstPage)).paintedPixels, {
                timeout: 30 * SECOND,
            })
            .toBeGreaterThan(0);
        const pixels = await renderedPixelCount(firstPage);
        expect(pixels.width).toBeGreaterThan(0);
        expect(pixels.height).toBeGreaterThan(0);
        expect(pixels.paintedPixels).toBeGreaterThan(0);

        const firstPageTextLayer = getPdfTextLayer(page, 1);
        await expect(firstPageTextLayer.locator('span')).not.toHaveCount(0, { timeout: 30 * SECOND });
        await expect.poll(() => selectedPdfText(page, 1), { timeout: 30 * SECOND }).not.toBe('');

        const firstPageLinks = getPdfAnnotationLayer(page, 1).locator('a');
        await expect(firstPageLinks).not.toHaveCount(0, { timeout: 30 * SECOND });
        const firstLink = firstPageLinks.first();
        await expect(firstLink).toHaveAttribute('href', /.+/);
        const firstLinkBox = await firstLink.boundingBox();
        expect(firstLinkBox?.width).toBeGreaterThan(0);
        expect(firstLinkBox?.height).toBeGreaterThan(0);
        await expect(firstLink).not.toHaveAttribute('target', '_blank');
        const pdfPages = page.locator('.pdf-viewer [data-layer="pages"]');
        const initialPdfScrollTop = await pdfPages.evaluate((node) => node.scrollTop);
        await firstLink.click();
        await expect
            .poll(async () => pdfPages.evaluate((node) => node.scrollTop), { timeout: 10 * SECOND })
            .toBeGreaterThan(initialPdfScrollTop);

        const zoomSlider = getPdfZoomSlider(page);
        const zoomValue = getPdfZoomValue(page);
        await expect(zoomSlider).toBeVisible();
        await expect(zoomSlider).toHaveAttribute('min', '-1');
        await expect(zoomSlider).toHaveAttribute('max', '1');
        await expect(zoomValue).toHaveText('100%');
        const sliderWidth = await zoomSlider.evaluate((node) => node.getBoundingClientRect().width);
        const viewerWidth = await page.locator('.pdf-viewer').evaluate((node) => node.getBoundingClientRect().width);
        expect(sliderWidth).toBeGreaterThan(viewerWidth * 0.28);
        expect(sliderWidth).toBeLessThan(viewerWidth * 0.32);

        const initialCssWidth = await renderedCssWidth(firstPage);
        const zoomSliderHandle = await zoomSlider.elementHandle();
        expect(zoomSliderHandle).not.toBeNull();
        await zoomSliderHandle.evaluate((node) => {
            node.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, pointerId: 1 }));
            window.__pdfZoomStreamDone = false;
            window.__pdfZoomStreamTimer = window.setInterval(() => {
                const zoom = 1 + 0.8 * Math.abs(Math.sin(Date.now() / 200));
                node.value = `${Math.log10(zoom)}`;
                node.dispatchEvent(new Event('input', { bubbles: true }));
            }, 20);
        });
        await expect
            .poll(async () => zoomSliderHandle.evaluate((node, minWidth) => {
                const canvas = node.ownerDocument.querySelector('.pdf-viewer canvas[data-page-number="1"]');
                const width = canvas?.getBoundingClientRect().width ?? 0;
                return !window.__pdfZoomStreamDone && width > minWidth ? width : 0;
            }, initialCssWidth * 1.15), { timeout: 30 * SECOND })
            .toBeGreaterThan(0);
        await zoomSliderHandle.evaluate((node) => {
            window.clearInterval(window.__pdfZoomStreamTimer);
            window.__pdfZoomStreamDone = true;
            node.dispatchEvent(new PointerEvent('pointerup', { bubbles: true, pointerId: 1 }));
        });

        await zoomSliderHandle.evaluate((node) => {
            node.value = `${Math.log10(2)}`;
            node.dispatchEvent(new Event('input', { bubbles: true }));
        });
        await expect(zoomValue).toHaveText('200%');
        await expect
            .poll(async () => renderedCssWidth(getPdfPage(page, 1)), { timeout: 30 * SECOND })
            .toBeGreaterThan(initialCssWidth * 1.8);

        const sliderCssWidth = await renderedCssWidth(getPdfPage(page, 1));
        const box = await firstPage.boundingBox();
        expect(box).not.toBeNull();
        await firstPage.dispatchEvent('wheel', {
            bubbles: true,
            cancelable: true,
            clientX: box.x + box.width / 2,
            clientY: box.y + box.height / 2,
            ctrlKey: true,
            deltaY: -600,
        });

        await expect
            .poll(async () => renderedCssWidth(getPdfPage(page, 1)), { timeout: 30 * SECOND })
            .toBeGreaterThan(sliderCssWidth * 1.05);
    });

    test('shows a git diff for modified files and returns to plain view when reverted', async ({ page }) => {
        test.setTimeout(120 * SECOND);

        const { baseDir, fileName, filePath } = await createCommittedReadme();

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });

        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        await expect(getMergeViewEditors(page)).toHaveCount(0, { timeout: 30 * SECOND });
        const plainEditor = getCodeMirrorContent(page).first();
        await expect(plainEditor).toContainText('Hello, World!', { timeout: 30 * SECOND });

        await replaceEditorText(page, plainEditor, 'Bonjour, Monde!');
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe('Bonjour, Monde!');

        await reloadFolder(page, `${baseDir}${path.sep}.`, fileName);

        await expect(getMergeViewEditors(page)).toHaveCount(2, { timeout: 30 * SECOND });
        const diffEditors = getCodeMirrorContent(page);
        await expect(diffEditors.nth(0)).toContainText('Hello, World!', { timeout: 30 * SECOND });
        await expect(diffEditors.nth(1)).toContainText('Bonjour, Monde!', { timeout: 30 * SECOND });

        await replaceEditorText(page, diffEditors.nth(1), 'Hello, World!');
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe('Hello, World!');

        await reloadFolder(page, baseDir, fileName);

        await expect(getMergeViewEditors(page)).toHaveCount(0, { timeout: 30 * SECOND });
        await expect(getCodeMirrorContent(page)).toHaveCount(1, { timeout: 30 * SECOND });
        await expect(getCodeMirrorContent(page).first()).toContainText('Hello, World!', { timeout: 30 * SECOND });
    });

    test('expands and collapses side-view folder nodes', async ({ page }) => {
        test.setTimeout(120 * SECOND);

        const root = await createFolderTree();

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });

        await setBasePath(page, root, 'a');
        await openFolderFile(page, 'a/');
        await openFolderFile(page, 'a2.txt');

        await expect(getCodeMirrorContent(page)).toContainText('I am Bob', { timeout: 30 * SECOND });
        await expect(getSideViewFile(page, 'a/a2.txt')).toBeVisible({ timeout: 30 * SECOND });
        await expect(getSideViewFile(page, 'a/a1.txt')).toHaveCount(0);

        const folderA = getSideViewFolder(page, 'a');
        await expect(folderA).toBeVisible({ timeout: 30 * SECOND });
        await folderA.hover();
        await folderA.locator('.side-view-expand-folder').click();

        await expect(getSideViewFile(page, 'a/a1.txt')).toBeVisible({ timeout: 30 * SECOND });
        await expect(getSideViewFolder(page, 'a/c')).toBeVisible({ timeout: 30 * SECOND });
        await expect(getSideViewFile(page, 'a/c/c.txt')).toHaveCount(0);

        await folderA.locator('.side-view-collapse-folder').click();

        await expect(getSideViewFile(page, 'a/a1.txt')).toHaveCount(0, { timeout: 30 * SECOND });
        await expect(getSideViewFolder(page, 'a/c')).toHaveCount(0, { timeout: 30 * SECOND });
    });

    test('creates files and folders from the folder toolbar', async ({ page }) => {
        test.setTimeout(120 * SECOND);

        const fileName = 'seed.txt';
        const { baseDir } = await createTempFile(fileName);

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });

        await setBasePath(page, baseDir, fileName);

        await getCreateFileIcon(page).click();
        await getCreateEntryField(page).fill(' notes with spaces.txt ');
        await getCreateEntryField(page).press('Enter');

        await expect(getFolderFile(page, 'notes with spaces.txt')).toBeVisible({ timeout: 30 * SECOND });
        await expect.poll(async () => readFile(path.join(baseDir, 'notes with spaces.txt'), 'utf8')).toBe('-- notes with spaces.txt --');

        await getCreateFolderIcon(page).click();
        await getCreateEntryField(page).fill(' drafts ');
        await getCreateEntryField(page).press('Enter');

        await expect.poll(async () => isDirectory(path.join(baseDir, 'drafts'))).toBe(true);
        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, 'drafts/');
        await expect(getFolderFile(page, 'drafts/')).toBeVisible({ timeout: 30 * SECOND });
    });

    test('moves a file to trash and resolves trash name conflicts', async ({ page }) => {
        test.setTimeout(120 * SECOND);

        const unique = `remove-me-${process.pid}-${Date.now()}`;
        const fileName = `${unique}.tar.gz`;
        const stem = unique;
        const today = todayUtc();
        const { baseDir, filePath } = await createTempFile(fileName);
        await writeFile(filePath, 'new');

        const trashDir = integrationTrashDir();
        await mkdir(trashDir, { recursive: true });
        await writeFile(path.join(trashDir, fileName), 'old');
        await writeFile(path.join(trashDir, `${stem}_${today}.tar.gz`), 'occupied');

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });

        await setBasePath(page, baseDir, fileName);

        await getFolderTrashIcon(page, fileName).click();

        await expect.poll(async () => exists(filePath), { timeout: 30 * SECOND }).toBe(false);
        await expect(getFolderFile(page, fileName)).toHaveCount(0, { timeout: 30 * SECOND });

        await expect
            .poll(async () => trashEntriesWithContent(trashDir, stem), { timeout: 30 * SECOND })
            .toEqual([
                [`${stem}_${today}-1.tar.gz`, 'old'],
                [`${stem}_${today}-2.tar.gz`, 'new'],
                [`${stem}_${today}.tar.gz`, 'occupied'],
            ]);
    });
});
