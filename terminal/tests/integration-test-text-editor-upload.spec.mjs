import { expect, test } from '@playwright/test';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
    BASE_URL,
    SECOND,
    authenticateApi,
    createTempDir,
    dropExternalFileIntoFolder,
    fsioApiUrl,
    getFolderDownloadIcon,
    getFolderFile,
    getSideViewFolder,
    refreshUntilFolderFileVisible,
    setBasePath,
} from './text-editor-helpers.mjs';

test.describe('Text editor upload', () => {
    test.describe.configure({ retries: 5 });

    test.beforeEach(async ({ page }) => {
        page.setDefaultTimeout(5 * SECOND);
        page.setDefaultNavigationTimeout(5 * SECOND);
    });

    test('uploads a file through the text editor API', async ({ request }) => {
        const baseDir = await createTempDir();
        const fileName = 'uploaded.bin';
        const content = Buffer.from([0, 1, 2, 3, 253, 254, 255]);

        await authenticateApi(request);
        const response = await request.post(fsioApiUrl('upload', baseDir, fileName), {
            data: content,
            headers: {
                'content-type': 'application/octet-stream',
            },
        });

        expect(response.status(), await response.text()).toBe(204);
        await expect.poll(async () => readFile(path.join(baseDir, fileName))).toEqual(content);
    });

    test('downloads a file through the text editor API', async ({ request }) => {
        const baseDir = await createTempDir();
        const fileName = 'downloaded.bin';
        const content = Buffer.from([255, 128, 64, 32, 16, 8, 0]);
        await writeFile(path.join(baseDir, fileName), content);

        await authenticateApi(request);
        const response = await request.get(fsioApiUrl('download', baseDir, fileName));

        expect(response.ok(), `download failed with status ${response.status()}: ${await response.text()}`).toBeTruthy();
        expect(response.headers()['content-type']).toMatch(/^application\/octet-stream\b/i);
        expect(await response.body()).toEqual(content);
    });

    test('downloads a file from the folder view', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const baseDir = await createTempDir();
        const fileName = 'download me.txt';
        const content = Buffer.from('download me');
        await writeFile(path.join(baseDir, fileName), content);
        await mkdir(path.join(baseDir, 'folder'), { recursive: true });

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);

        const downloadIcon = getFolderDownloadIcon(page, fileName);
        await expect(downloadIcon).toBeVisible({ timeout: 10 * SECOND });
        await expect(getFolderDownloadIcon(page, 'folder/')).toHaveCount(0);

        const link = downloadIcon.locator('xpath=ancestor::a[1]');
        await expect(link).toHaveAttribute('download', fileName);

        const href = await link.getAttribute('href');
        expect(href).toBeTruthy();
        const url = new URL(href, BASE_URL);
        expect(url.pathname).toBe('/api/text_editor/fsio/download');
        expect(url.searchParams.get('base')).toBe(baseDir);
        expect(url.searchParams.get('file')).toBe(fileName);

        const [download] = await Promise.all([
            page.waitForEvent('download'),
            downloadIcon.click(),
        ]);
        expect(download.suggestedFilename()).toBe(fileName);

        const downloadedPath = path.join(process.env.TEST_TMPDIR ?? tmpdir(), `downloaded-${process.pid}-${Date.now()}-${fileName}`);
        await download.saveAs(downloadedPath);
        expect(await readFile(downloadedPath)).toEqual(content);
    });

    test('uploads a dropped file into a side-view folder', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const baseDir = await createTempDir();
        await writeFile(path.join(baseDir, 'seed.txt'), 'seed');

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, 'seed.txt');

        const fileName = 'uploaded from finder.txt';
        const content = 'Dropped from Finder or File Explorer';
        await dropExternalFileIntoFolder(page, getSideViewFolder(page, ''), fileName, content);

        const uploadedPath = path.join(baseDir, fileName);
        await expect.poll(async () => readFile(uploadedPath, 'utf8'), { timeout: 10 * SECOND }).toBe(content);
        await refreshUntilFolderFileVisible(page, baseDir, fileName);
    });
});
