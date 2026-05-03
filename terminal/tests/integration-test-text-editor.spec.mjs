import { expect, test } from '@playwright/test';
import { copyFile, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

const SECOND = 1000;
const BASE_URL = process.env.BASE_URL ?? 'http://127.0.0.1:3000';
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

function getPdfPage(page, pageNumber) {
    return page.locator(`.pdf-viewer canvas[data-page-number="${pageNumber}"]`);
}

function getPdfTextLayer(page, pageNumber) {
    return page.locator(`.pdf-viewer > div[data-page-number="${pageNumber}"] [data-layer="text"]`);
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
            `.pdf-viewer > div[data-page-number="${pageNumber}"] [data-layer="text"]`,
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

async function showBasePathInput(page) {
    const basePathInput = getBasePathInput(page);
    await page
        .locator('.base-path-selector-field, .base-path-selector-display')
        .first()
        .waitFor({ state: 'visible', timeout: 30 * SECOND });
    if (!(await basePathInput.isVisible().catch(() => false))) {
        await getBasePathDisplay(page).dblclick();
    }
    await expect(basePathInput).toBeVisible({ timeout: 30 * SECOND });
    return basePathInput;
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

        const basePathInput = await showBasePathInput(page);
        await basePathInput.fill(baseDir);
        await basePathInput.blur();

        await getFolderFile(page, fileName).click();

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

        const basePathInput = await showBasePathInput(page);
        await basePathInput.fill(baseDir);
        await basePathInput.blur();

        await getFolderFile(page, 'PlantUML.pdf').click();

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
    });
});
