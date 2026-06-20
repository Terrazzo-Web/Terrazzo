import { expect } from '@playwright/test';
import { execFile } from 'node:child_process';
import { copyFile, mkdir, mkdtemp, readFile, readdir, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';

export const SECOND = 1000;
const execFileAsync = promisify(execFile);
export const BASE_URL = (process.env.BASE_URL ?? 'http://127.0.0.1:3000')
    .split(';')
    .map((url) => url.trim())
    .filter(Boolean)[0];
const TEXT_EDITOR_FSIO_URL = `${BASE_URL}/api/text_editor/fsio`;
const WORKSPACE_ROOT = path.join(process.env.TEST_SRCDIR ?? '.', process.env.TEST_WORKSPACE ?? '.');
export const PLANTUML_PDF = path.join(WORKSPACE_ROOT, 'terminal/tests/PlantUML.pdf');

export async function createTempFile(name) {
    const baseDir = await mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'text-editor-'));
    const filePath = path.join(baseDir, name);
    await writeFile(filePath, '');
    return { baseDir, filePath };
}

export async function createTempDir() {
    return mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'text-editor-api-'));
}

export async function authenticateApi(request) {
    const response = await request.post(`${BASE_URL}/api/login`, { data: null });
    expect(response.ok(), `login failed with status ${response.status()}: ${await response.text()}`).toBeTruthy();
}

export function fsioApiUrl(action, base, file) {
    const params = new URLSearchParams({ base, file });
    return `${TEXT_EDITOR_FSIO_URL}/${action}?${params}`;
}

function getBasePathInput(page) {
    return page.locator('.base-path-selector-field');
}

function getBasePathDisplay(page) {
    return page.locator('.base-path-selector-display');
}

export function getCodeMirrorContent(page) {
    return page.locator('.code-mirror-editor .cm-content');
}

export function getCodeMirrorSearchPanel(page) {
    return page.locator('.code-mirror-editor .cm-search');
}

export function getMergeViewEditors(page) {
    return page.locator('.code-mirror-editor .cm-mergeViewEditor');
}

export function editorFindShortcut() {
    return process.platform === 'darwin' ? 'Meta+F' : 'Control+F';
}

export function getSideViewFile(page, filePath) {
    return page.locator(`.side-view [data-file-path="${filePath}"]`);
}

export function getSideViewFolder(page, folderPath) {
    return page.locator(`.side-view [data-folder-path="${folderPath}"] .side-view-folder-row`);
}

export function getFolderFile(page, name) {
    return page.locator('.folder-row', { has: page.locator('.folder-name', { hasText: name }) });
}

async function listDiskFolderEntries(baseDir) {
    const entries = await readdir(baseDir, { withFileTypes: true });
    return entries.map((entry) => `${entry.name}${entry.isDirectory() ? '/' : ''}`).sort();
}

async function listDisplayedFolderEntries(page) {
    return page.locator('.folder-row .folder-name').allTextContents();
}

function expectedDiskEntryVariants(expectedFileName) {
    if (expectedFileName.endsWith('/')) {
        return [expectedFileName];
    }
    return [expectedFileName, `${expectedFileName}/`];
}

export function getFolderTrashIcon(page, name) {
    return getFolderFile(page, name).locator('.folder-trash-icon');
}

export function getFolderDownloadIcon(page, name) {
    return getFolderFile(page, name).locator('.folder-download-icon');
}

export function getPdfPage(page, pageNumber) {
    return page.locator(`.pdf-viewer canvas[data-page-number="${pageNumber}"]`);
}

export function getPdfTextLayer(page, pageNumber) {
    return page.locator(`.pdf-viewer [data-layer="pages"] > div[data-page-number="${pageNumber}"] [data-layer="text"]`);
}

export function getPdfAnnotationLayer(page, pageNumber) {
    return page.locator(
        `.pdf-viewer [data-layer="pages"] > div[data-page-number="${pageNumber}"] [data-layer="annotations"]`,
    );
}

export function getPdfZoomSlider(page) {
    return page.locator('.pdf-viewer [data-control="zoom"] input[type="range"]');
}

export function getPdfZoomValue(page) {
    return page.locator('.pdf-viewer [data-control="zoom"] output');
}

export function getHtmlViewerFrame(page) {
    return page.locator('.html-viewer iframe[data-viewer="html"]');
}

export function getCreateFileIcon(page) {
    return page.locator('.create-file-icon');
}

export function getCreateFolderIcon(page) {
    return page.locator('.create-folder-icon');
}

export function getCreateEntryField(page) {
    return page.locator('.create-entry-field');
}

export async function expectPdfPage(page, pageNumber) {
    const viewer = page.locator('.pdf-viewer');
    await expect(viewer).toBeVisible({ timeout: 10 * SECOND });

    const canvas = getPdfPage(page, pageNumber);
    try {
        await expect(canvas).toBeVisible({ timeout: 10 * SECOND });
    } catch (error) {
        const status = await viewer.locator('.pdf-status').textContent().catch(() => '<none>');
        throw new Error(`PDF page ${pageNumber} did not render. Status: ${status}\n${error.message}`);
    }
    return canvas;
}

export async function selectedPdfText(page, pageNumber) {
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

export async function renderedCssWidth(canvas) {
    return canvas.evaluate((node) => node.getBoundingClientRect().width);
}

export async function renderedPixelCount(canvas) {
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

export async function showBasePathInput(page, timeout = 10 * SECOND) {
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

export async function setBasePath(page, baseDir, expectedFileName, timeout = 10 * SECOND) {
    const diskEntries = await listDiskFolderEntries(baseDir);
    expect(
        diskEntries.some((entry) => expectedDiskEntryVariants(expectedFileName).includes(entry)),
        `Expected ${expectedFileName} to exist on disk in ${baseDir}; disk entries: ${JSON.stringify(diskEntries)}`,
    ).toBe(true);

    try {
        await page.waitForTimeout(SECOND);
        const basePathInput = await showBasePathInput(page, timeout);
        await basePathInput.fill(baseDir);
        await page.keyboard.press('Tab');
        await page.waitForTimeout(SECOND);
        await expect(getFolderFile(page, expectedFileName)).toBeVisible({ timeout });
    } catch (error) {
        const displayedEntries = await listDisplayedFolderEntries(page).catch((listError) => [
            `<failed to read displayed entries: ${listError.message}>`,
        ]);
        console.log(
            [
                `setBasePath failed for ${baseDir}`,
                `Expected file: ${expectedFileName}`,
                `Disk entries: ${JSON.stringify(diskEntries)}`,
                `Displayed entries: ${JSON.stringify(displayedEntries)}`,
            ].join('\n'),
        );
        throw error;
    }
}

export async function refreshUntilFolderFileVisible(page, baseDir, expectedFileName, timeout = 10 * SECOND) {
    const diskEntries = await listDiskFolderEntries(baseDir);
    expect(
        diskEntries.some((entry) => expectedDiskEntryVariants(expectedFileName).includes(entry)),
        `Expected ${expectedFileName} to exist on disk in ${baseDir}; disk entries: ${JSON.stringify(diskEntries)}`,
    ).toBe(true);

    try {
        await page.waitForTimeout(SECOND);
        await page.reload({ waitUntil: 'domcontentloaded' });
        await expect(getFolderFile(page, expectedFileName)).toBeVisible({ timeout });
    } catch (error) {
        const displayedEntries = await listDisplayedFolderEntries(page).catch((listError) => [
            `<failed to read displayed entries: ${listError.message}>`,
        ]);
        console.log(
            [
                `refreshUntilFolderFileVisible failed for ${baseDir}`,
                `Expected file: ${expectedFileName}`,
                `Disk entries: ${JSON.stringify(diskEntries)}`,
                `Displayed entries: ${JSON.stringify(displayedEntries)}`,
            ].join('\n'),
        );
        throw error;
    }
}

export async function openFolderFile(page, name, timeout = 10 * SECOND) {
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

export async function reopenFolderFile(page, fileName) {
    await getSideViewFolder(page, '').locator('span').first().click();
    await expect(getFolderFile(page, fileName)).toBeVisible({ timeout: 10 * SECOND });
    await openFolderFile(page, fileName);
}

async function git(cwd, args) {
    await execFileAsync('git', args, { cwd });
}

export async function createCommittedReadme() {
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

export async function createFolderTree() {
    const root = await mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'text-editor-tree-'));
    await mkdir(path.join(root, 'a', 'c'), { recursive: true });
    await mkdir(path.join(root, 'b'), { recursive: true });
    await writeFile(path.join(root, 'a', 'a1.txt'), 'I am Alice');
    await writeFile(path.join(root, 'a', 'a2.txt'), 'I am Bob');
    await writeFile(path.join(root, 'a', 'c', 'c.txt'), 'I am Charlie');
    return root;
}

export async function createMoveTree() {
    const root = await mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'text-editor-move-'));
    await mkdir(path.join(root, 'destfolder'), { recursive: true });
    await mkdir(path.join(root, 'srcfolder', 'src_folder'), { recursive: true });
    await writeFile(path.join(root, 'srcfolder', 'src_file'), 'move me');
    await writeFile(path.join(root, 'srcfolder', 'src_folder', 'child.txt'), 'move the folder too');
    return root;
}

export async function exists(filePath) {
    return stat(filePath)
        .then(() => true)
        .catch(() => false);
}

export async function isDirectory(filePath) {
    return stat(filePath)
        .then((metadata) => metadata.isDirectory())
        .catch(() => false);
}

async function readFileOrMissing(filePath) {
    return readFile(filePath, 'utf8').catch(() => '<missing>');
}

export async function trashEntriesWithContent(trashDir, stem) {
    const entries = await readdir(trashDir).catch(() => []);
    const matching = [];
    for (const entry of entries.filter((entry) => entry.startsWith(stem))) {
        matching.push([entry, await readFileOrMissing(path.join(trashDir, entry))]);
    }
    return matching.sort(([a], [b]) => a.localeCompare(b));
}

export function todayUtc() {
    return new Date().toISOString().slice(0, 10);
}

export function integrationTrashDir() {
    return path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'terrazzo-integration-test', 'trash');
}

export async function replaceEditorText(page, editor, content) {
    await editor.click();
    await page.keyboard.press(process.platform === 'darwin' ? 'Meta+A' : 'Control+A');
    await page.keyboard.insertText(content);
}

export async function dragSideViewNodeIntoFolder(page, source, sourcePath, destinationFolder) {
    await source.scrollIntoViewIfNeeded();
    await destinationFolder.scrollIntoViewIfNeeded();
    expect(sourcePath).toBeTruthy();
    const dataTransfer = await page.evaluateHandle((sourcePath) => {
        const dataTransfer = new DataTransfer();
        dataTransfer.setData('text-editor-move-file', sourcePath);
        dataTransfer.effectAllowed = 'move';
        return dataTransfer;
    }, sourcePath);
    await destinationFolder.dispatchEvent('dragover', { dataTransfer });
    await destinationFolder.dispatchEvent('drop', { dataTransfer });
    await dataTransfer.dispose();
    await page.waitForTimeout(SECOND);
}

export async function dropExternalFileIntoFolder(page, destinationFolder, fileName, content) {
    await destinationFolder.scrollIntoViewIfNeeded();
    const dataTransfer = await page.evaluateHandle(
        ({ fileName, content }) => {
            const dataTransfer = new DataTransfer();
            dataTransfer.items.add(new File([content], fileName, { type: 'text/plain' }));
            dataTransfer.effectAllowed = 'copy';
            return dataTransfer;
        },
        { fileName, content },
    );
    await destinationFolder.dispatchEvent('dragover', { dataTransfer });
    await destinationFolder.dispatchEvent('drop', { dataTransfer });
    await dataTransfer.dispose();
}

export async function expandSideViewFolder(page, folderPath) {
    const folder = getSideViewFolder(page, folderPath);
    await expect(folder).toBeVisible({ timeout: 10 * SECOND });
    await folder.hover();
    const expandIcon = folder.locator('.side-view-expand-folder');
    if (await expandIcon.isVisible().catch(() => false)) {
        await expandIcon.click();
        await page.waitForTimeout(SECOND);
    }
}

export async function copyPlantUmlPdf(destination) {
    await copyFile(PLANTUML_PDF, destination);
}
