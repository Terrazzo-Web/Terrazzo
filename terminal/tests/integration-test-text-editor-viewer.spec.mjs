import { expect, test } from '@playwright/test';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
    BASE_URL,
    SECOND,
    copyPlantUmlPdf,
    expectPdfPage,
    getCodeMirrorContent,
    getHtmlViewerFrame,
    getPdfAnnotationLayer,
    getPdfPage,
    getPdfTextLayer,
    getPdfZoomSlider,
    getPdfZoomValue,
    getSideViewFile,
    openFolderFile,
    renderedCssWidth,
    renderedPixelCount,
    selectedPdfText,
    setBasePath,
} from './text-editor-helpers.mjs';

test.describe('Text editor viewers', () => {
    test.describe.configure({ retries: 5 });

    test.beforeEach(async ({ page }) => {
        page.setDefaultTimeout(5 * SECOND);
        page.setDefaultNavigationTimeout(5 * SECOND);
    });

    test('renders a PDF file', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const baseDir = await mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'text-editor-pdf-'));
        await copyPlantUmlPdf(path.join(baseDir, 'PlantUML.pdf'));

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, 'PlantUML.pdf');
        await openFolderFile(page, 'PlantUML.pdf');

        await expect(getSideViewFile(page, 'PlantUML.pdf')).toBeVisible({ timeout: 10 * SECOND });

        const firstPage = await expectPdfPage(page, 1);
        await expect
            .poll(async () => (await renderedPixelCount(firstPage)).paintedPixels, {
                timeout: 10 * SECOND,
            })
            .toBeGreaterThan(0);
        const pixels = await renderedPixelCount(firstPage);
        expect(pixels.width).toBeGreaterThan(0);
        expect(pixels.height).toBeGreaterThan(0);
        expect(pixels.paintedPixels).toBeGreaterThan(0);

        const firstPageTextLayer = getPdfTextLayer(page, 1);
        await expect(firstPageTextLayer.locator('span')).not.toHaveCount(0, { timeout: 10 * SECOND });
        await expect.poll(() => selectedPdfText(page, 1), { timeout: 10 * SECOND }).not.toBe('');

        const firstPageLinks = getPdfAnnotationLayer(page, 1).locator('a');
        await expect(firstPageLinks).not.toHaveCount(0, { timeout: 10 * SECOND });
        const firstLink = firstPageLinks.first();
        await expect(firstLink).toHaveAttribute('href', /.+/);
        await expect(firstLink).not.toHaveAttribute('target', '_blank');

        const zoomSlider = getPdfZoomSlider(page);
        const zoomValue = getPdfZoomValue(page);
        await expect(zoomSlider).toBeVisible();
        await expect(zoomSlider).toHaveAttribute('min', '-1');
        await expect(zoomSlider).toHaveAttribute('max', '1');
        await expect(zoomValue).toHaveText('100%');
        const sliderWidth = await zoomSlider.evaluate((node) => node.getBoundingClientRect().width);
        expect(sliderWidth).toBeGreaterThan(100);
        expect(sliderWidth).toBeLessThan(160);

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
            }, initialCssWidth * 1.15), { timeout: 10 * SECOND })
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
            .poll(async () => renderedCssWidth(getPdfPage(page, 1)), { timeout: 10 * SECOND })
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
            .poll(async () => renderedCssWidth(getPdfPage(page, 1)), { timeout: 10 * SECOND })
            .toBeGreaterThan(sliderCssWidth * 1.05);
    });

    test('renders an HTML file in an iframe', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const baseDir = await mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'text-editor-html-'));
        const fileName = 'preview.html';
        await mkdir(path.join(baseDir, 'nested'), { recursive: true });
        await writeFile(
            path.join(baseDir, fileName),
            [
                '<!doctype html>',
                '<html>',
                '<head><title>Preview fixture</title><style>body{font-family:sans-serif} .ready{color:rgb(12, 91, 42)}</style></head>',
                '<body><main><h1>HTML preview works</h1><p class="ready">Rendered inside the iframe</p></main></body>',
                '</html>',
            ].join('\n'),
        );

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);

        await expect(getSideViewFile(page, fileName)).toBeVisible({ timeout: 10 * SECOND });
        const frame = getHtmlViewerFrame(page);
        await expect(frame).toBeVisible({ timeout: 10 * SECOND });
        await expect(frame).toHaveAttribute('sandbox', '');
        const htmlPreviewToggle = page.locator('.toggle-html-preview');
        await expect(htmlPreviewToggle).toBeVisible({ timeout: 10 * SECOND });
        await expect(htmlPreviewToggle).toHaveAttribute('title', 'Show HTML source');

        await expect
            .poll(async () => {
                const contentFrame = await frame.elementHandle().then((handle) => handle?.contentFrame());
                return contentFrame?.locator('body').textContent();
            }, { timeout: 10 * SECOND })
            .toContain('HTML preview works');

        const contentFrame = await frame.elementHandle().then((handle) => handle?.contentFrame());
        expect(contentFrame).not.toBeNull();
        await expect(contentFrame.locator('h1')).toHaveText('HTML preview works');
        await expect(contentFrame.locator('.ready')).toHaveText('Rendered inside the iframe');

        await htmlPreviewToggle.evaluate((node) => node.click());
        await expect(htmlPreviewToggle).toHaveAttribute('title', 'Preview HTML');
        await expect(getHtmlViewerFrame(page)).toHaveCount(0, { timeout: 10 * SECOND });
        await expect(getCodeMirrorContent(page)).toContainText('HTML preview works', { timeout: 10 * SECOND });

        await htmlPreviewToggle.evaluate((node) => node.click());
        await expect(htmlPreviewToggle).toHaveAttribute('title', 'Show HTML source');
        await expect(getHtmlViewerFrame(page)).toBeVisible({ timeout: 10 * SECOND });
    });
});
