import { expect, test } from '@playwright/test';
import { execFile } from 'node:child_process';
import { copyFile, mkdtemp, readFile, writeFile } from 'node:fs/promises';
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

function getFolderFile(page, name) {
    return page.locator('.folder-row', { has: page.locator('.folder-name', { hasText: name }) });
}

function getCodeMirrorContent(page) {
    return page.locator('.code-mirror-editor .cm-content');
}

function getSideViewFile(page, filePath) {
    return page.locator(`.side-view [data-file-path="${filePath}"]`);
}

function getMergeViewEditors(page) {
    return page.locator('.code-mirror-editor .cm-mergeViewEditor');
}

function getPdfPage(page, pageNumber) {
    return page.locator(`.pdf-viewer canvas[data-page-number="${pageNumber}"]`);
}

function getPdfTextLayer(page, pageNumber) {
    return page.locator(`.pdf-viewer [data-layer="pages"] > div[data-page-number="${pageNumber}"] [data-layer="text"]`);
}

function getPdfZoomSlider(page) {
    return page.locator('.pdf-viewer [data-control="zoom"] input[type="range"]');
}

function getPdfZoomValue(page) {
    return page.locator('.pdf-viewer [data-control="zoom"] output');
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

async function setBasePath(page, baseDir, expectedFileName) {
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
            { timeout: 30 * SECOND },
        )
        .toBe(true);
}

async function openFolderFile(page, name) {
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
            { timeout: 30 * SECOND },
        )
        .toBe(true);
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

async function replaceEditorText(page, editor, content) {
    await editor.click();
    await page.keyboard.press(process.platform === 'darwin' ? 'Meta+A' : 'Control+A');
    await page.keyboard.type(content);
}

async function closeSideViewFile(page, filePath) {
    const sideViewFile = getSideViewFile(page, filePath);
    await expect(sideViewFile).toBeVisible({ timeout: 30 * SECOND });
    await sideViewFile.hover();
    await sideViewFile.locator('img.side-view-close-file').click({ force: true });
    await expect(sideViewFile).toHaveCount(0, { timeout: 30 * SECOND });
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

    test('renders a PDF file', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const baseDir = await mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'text-editor-pdf-'));
        await copyFile(PLANTUML_PDF, path.join(baseDir, 'PlantUML.pdf'));

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });

        await setBasePath(page, baseDir, 'PlantUML.pdf');

        await openFolderFile(page, 'PlantUML.pdf');

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
        test.setTimeout(60 * SECOND);

        const { baseDir, fileName, filePath } = await createCommittedReadme();

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });

        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        await expect(getMergeViewEditors(page)).toHaveCount(0, { timeout: 30 * SECOND });
        const plainEditor = getCodeMirrorContent(page).first();
        await expect(plainEditor).toContainText('Hello, World!', { timeout: 30 * SECOND });

        await replaceEditorText(page, plainEditor, 'Bonjour, Monde!');
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe('Bonjour, Monde!');

        await closeSideViewFile(page, fileName);
        await openFolderFile(page, fileName);

        await expect(getMergeViewEditors(page)).toHaveCount(2, { timeout: 30 * SECOND });
        const diffEditors = getCodeMirrorContent(page);
        await expect(diffEditors.nth(0)).toContainText('Hello, World!', { timeout: 30 * SECOND });
        await expect(diffEditors.nth(1)).toContainText('Bonjour, Monde!', { timeout: 30 * SECOND });

        await replaceEditorText(page, diffEditors.nth(1), 'Hello, World!');
        await expect.poll(async () => readFile(filePath, 'utf8'), { timeout: 10 * SECOND }).toBe('Hello, World!');

        await closeSideViewFile(page, fileName);
        await openFolderFile(page, fileName);

        await expect(getMergeViewEditors(page)).toHaveCount(0, { timeout: 30 * SECOND });
        await expect(getCodeMirrorContent(page)).toHaveCount(1, { timeout: 30 * SECOND });
        await expect(getCodeMirrorContent(page).first()).toContainText('Hello, World!', { timeout: 30 * SECOND });
    });
});
