import { expect, test } from '@playwright/test';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

import {
    BASE_URL,
    SECOND,
    createMoveTree,
    createTempFile,
    dragSideViewNodeIntoFolder,
    exists,
    expandSideViewFolder,
    getCreateEntryField,
    getCreateFileIcon,
    getCreateFolderIcon,
    getFolderFile,
    getFolderTrashIcon,
    getSideViewFile,
    getSideViewFolder,
    integrationTrashDir,
    isDirectory,
    openFolderFile,
    refreshUntilFolderFileVisible,
    setBasePath,
    todayUtc,
    trashEntriesWithContent,
} from './text-editor-helpers.mjs';

test.describe('Text editor file ops', () => {
    test.describe.configure({ retries: 5 });

    test.beforeEach(async ({ page }) => {
        page.setDefaultTimeout(5 * SECOND);
        page.setDefaultNavigationTimeout(5 * SECOND);
    });

    test('creates files and folders from the folder toolbar', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const fileName = 'seed.txt';
        const { baseDir } = await createTempFile(fileName);

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);

        await getCreateFileIcon(page).click();
        await getCreateEntryField(page).fill(' notes with spaces.txt ');
        await getCreateEntryField(page).press('Enter');

        await expect(getFolderFile(page, 'notes with spaces.txt')).toBeVisible({ timeout: 10 * SECOND });
        await expect.poll(async () => readFile(path.join(baseDir, 'notes with spaces.txt'), 'utf8')).toBe('-- notes with spaces.txt --');

        await getCreateFolderIcon(page).click();
        await getCreateEntryField(page).fill(' drafts ');
        await getCreateEntryField(page).press('Enter');

        await expect.poll(async () => isDirectory(path.join(baseDir, 'drafts'))).toBe(true);
        await refreshUntilFolderFileVisible(page, baseDir, 'drafts/');
        await expect(getFolderFile(page, 'drafts/')).toBeVisible({ timeout: 10 * SECOND });
    });

    test('moves files and folders by dragging side-view nodes into folder nodes', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const baseDir = await createMoveTree();

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, 'destfolder/');
        await openFolderFile(page, 'destfolder/');
        await getSideViewFolder(page, '').locator('span').first().click();
        await expect(getFolderFile(page, 'srcfolder/')).toBeVisible({ timeout: 10 * SECOND });
        await openFolderFile(page, 'srcfolder/');
        await expandSideViewFolder(page, '');
        await expandSideViewFolder(page, 'srcfolder');

        await expect(getSideViewFolder(page, 'destfolder')).toBeVisible({ timeout: 10 * SECOND });
        await expect(getSideViewFile(page, 'srcfolder/src_file')).toBeVisible({ timeout: 10 * SECOND });
        await expect(getSideViewFolder(page, 'srcfolder/src_folder')).toBeVisible({ timeout: 10 * SECOND });

        await dragSideViewNodeIntoFolder(
            page,
            getSideViewFile(page, 'srcfolder/src_file'),
            path.join(baseDir, 'srcfolder', 'src_file'),
            getSideViewFolder(page, 'destfolder'),
        );

        await expect.poll(async () => exists(path.join(baseDir, 'destfolder', 'src_file')), { timeout: 10 * SECOND }).toBe(true);
        await expect.poll(async () => exists(path.join(baseDir, 'srcfolder', 'src_file')), { timeout: 10 * SECOND }).toBe(false);
        await page.waitForTimeout(SECOND);
        await expect(getSideViewFile(page, 'srcfolder/src_file')).toHaveCount(0, { timeout: 10 * SECOND });

        await dragSideViewNodeIntoFolder(
            page,
            getSideViewFolder(page, 'srcfolder/src_folder'),
            path.join(baseDir, 'srcfolder', 'src_folder'),
            getSideViewFolder(page, 'destfolder'),
        );

        await expect.poll(async () => isDirectory(path.join(baseDir, 'destfolder', 'src_folder')), { timeout: 10 * SECOND }).toBe(true);
        await expect.poll(async () => exists(path.join(baseDir, 'srcfolder', 'src_folder')), { timeout: 10 * SECOND }).toBe(false);
        await page.waitForTimeout(SECOND);
        await expect(getSideViewFolder(page, 'srcfolder/src_folder')).toHaveCount(0, { timeout: 10 * SECOND });
    });

    test('moves a file to trash and resolves trash name conflicts', async ({ page }) => {
        test.setTimeout(60 * SECOND);

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

        await expect.poll(async () => exists(filePath), { timeout: 10 * SECOND }).toBe(false);
        await expect(getFolderFile(page, fileName)).toHaveCount(0, { timeout: 10 * SECOND });

        await expect
            .poll(async () => trashEntriesWithContent(trashDir, stem), { timeout: 10 * SECOND })
            .toEqual([
                [`${stem}_${today}-1.tar.gz`, 'old'],
                [`${stem}_${today}-2.tar.gz`, 'new'],
                [`${stem}_${today}.tar.gz`, 'occupied'],
            ]);
    });

    test('removes a deleted file from the side view', async ({ page }) => {
        test.setTimeout(60 * SECOND);

        const fileName = `side-view-remove-me-${process.pid}-${Date.now()}.txt`;
        const { baseDir, filePath } = await createTempFile(fileName);
        await writeFile(filePath, 'delete me');

        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await setBasePath(page, baseDir, fileName);
        await openFolderFile(page, fileName);
        await expect(getSideViewFile(page, fileName)).toBeVisible({ timeout: 10 * SECOND });

        await getSideViewFolder(page, '').locator('span').first().click();
        await expect(getFolderFile(page, fileName)).toBeVisible({ timeout: 10 * SECOND });
        await getFolderTrashIcon(page, fileName).click();

        await expect.poll(async () => exists(filePath), { timeout: 10 * SECOND }).toBe(false);
        await expect(getFolderFile(page, fileName)).toHaveCount(0, { timeout: 10 * SECOND });
        await expect(getSideViewFile(page, fileName)).toHaveCount(0, { timeout: 10 * SECOND });
    });
});
