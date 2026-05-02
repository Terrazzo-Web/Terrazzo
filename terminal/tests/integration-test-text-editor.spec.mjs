import { expect, test } from '@playwright/test';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

const SECOND = 1000;
const BASE_URL = process.env.BASE_URL ?? 'http://127.0.0.1:3000';

async function createTempFile(name) {
  const baseDir = await mkdtemp(path.join(process.env.TEST_TMPDIR ?? tmpdir(), 'text-editor-'));
  const filePath = path.join(baseDir, name);
  await writeFile(filePath, '');
  return { baseDir, filePath };
}

function getBasePathInput(page) {
  return page.locator('.base-path-selector-field');
}

function getFolderFile(page, name) {
  return page.locator('.folder-row', { has: page.locator('.folder-name', { hasText: name }) });
}

function getCodeMirrorContent(page) {
  return page.locator('.code-mirror-editor .cm-content');
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

    const basePathInput = getBasePathInput(page);
    await expect(basePathInput).toBeVisible({ timeout: 30 * SECOND });
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
});
