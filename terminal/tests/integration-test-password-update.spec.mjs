import { expect, test } from '@playwright/test';
import { randomUUID } from 'node:crypto';
import { spawn } from 'node:child_process';

const SECOND = 1000;
const BASE_URL = process.env.BASE_URL ?? 'http://127.0.0.1:3000';
const SERVER_BIN = process.env.TERRAZZO_SERVER_BIN;
const CONFIG_FILE = process.env.TERRAZZO_CONFIG_FILE;

function getAddTabButton(page) {
  return page.locator('div[class*="add-tab-icon-"] img');
}

function getPasswordInput(page) {
  return page.locator('input[type="password"]');
}

async function setPassword(password) {
  expect(SERVER_BIN, 'TERRAZZO_SERVER_BIN must be set').toBeTruthy();
  expect(CONFIG_FILE, 'TERRAZZO_CONFIG_FILE must be set').toBeTruthy();

  await new Promise((resolve, reject) => {
    const child = spawn(
      SERVER_BIN,
      ['--config-file', CONFIG_FILE, '--action', 'set-password', '--password-stdin'],
      {
        env: {
          ...process.env,
          RUST_BACKTRACE: '1',
        },
        stdio: ['pipe', 'pipe', 'pipe'],
      },
    );

    let stderr = '';
    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString();
    });
    child.on('error', reject);
    child.on('exit', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`set-password exited with code ${code}: ${stderr}`));
      }
    });

    child.stdin.end(`${password}\n`);
  });
}

async function reloadUntilPasswordLogin(page) {
  const passwordInput = getPasswordInput(page);
  for (let attempt = 0; attempt < 6; attempt += 1) {
    await page.reload({ waitUntil: 'domcontentloaded' });
    if (await passwordInput.isVisible().catch(() => false)) {
      return;
    }
    await page.waitForTimeout(SECOND);
  }

  await expect(
    passwordInput,
    'password login should appear after live config reload applies the updated password',
  ).toBeVisible({ timeout: 5 * SECOND });
}

test.describe('Password update', () => {
  test.beforeEach(async ({ page }) => {
    page.setDefaultTimeout(5 * SECOND);
    page.setDefaultNavigationTimeout(5 * SECOND);
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
  });

  test('requires login after setting a password via CLI', async ({ page }) => {
    const addTabButton = getAddTabButton(page);
    await expect(addTabButton).toBeVisible({ timeout: 30 * SECOND });

    const password = `trz-${randomUUID()}`;
    await setPassword(password);

    await reloadUntilPasswordLogin(page);

    const passwordInput = getPasswordInput(page);
    await expect(passwordInput).toBeVisible();
    await passwordInput.fill(password);
    await passwordInput.dispatchEvent('change');
    await expect(addTabButton).toBeVisible({ timeout: 30 * SECOND });
  });
});
