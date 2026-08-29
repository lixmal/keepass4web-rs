// A database opened with a key file has to be saved with the same key file.
// Saving with only the master password re-encrypts it to the password alone:
// the key file requirement is dropped without anyone being told, and the owner
// who still presents the key file can no longer open their own database.

const { execFileSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const { test, expect } = require('@playwright/test');
const { LOGIN_TIMEOUT, MASTER_PASSWORD } = require('./helpers');

const FIXTURE = path.join(__dirname, '..', 'test.kdbx');
const DB = path.join(__dirname, '..', 'tmp', 'compat.kdbx');
const KEYFILE = path.join(__dirname, '..', 'tmp', 'compat.key');

function kxc(flags, positional = [], input = `${MASTER_PASSWORD}\n`) {
  return execFileSync('keepassxc-cli', [...flags, DB, ...positional], {
    input,
    encoding: 'utf8',
    stdio: ['pipe', 'pipe', 'pipe'],
  });
}

// whether keepassxc can open the database with the credentials given
function opens(withKeyfile) {
  const flags = withKeyfile ? ['ls', '-q', '-k', KEYFILE] : ['ls', '-q'];
  try {
    execFileSync('keepassxc-cli', [...flags, DB], {
      input: `${MASTER_PASSWORD}\n`,
      stdio: ['pipe', 'ignore', 'ignore'],
    });
    return true;
  } catch {
    return false;
  }
}

const installed = (() => {
  try {
    execFileSync('keepassxc-cli', ['--version'], { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
})();

test.use({ baseURL: 'http://localhost:8182' });

test.describe('A database that needs a key file', () => {
  test.skip(!installed, 'keepassxc-cli is not installed');

  test.beforeEach(async () => {
    fs.mkdirSync(path.dirname(DB), { recursive: true });
    fs.copyFileSync(FIXTURE, DB);

    // a fresh random key file, added to the fixture
    fs.writeFileSync(KEYFILE, require('crypto').randomBytes(64));
    kxc(['db-edit', '--set-key-file', KEYFILE]);

    expect(opens(false)).toBe(false);
    expect(opens(true)).toBe(true);
  });

  async function unlock(page) {
    await page.goto('/');
    await page.waitForURL(/\/db_login/, { timeout: LOGIN_TIMEOUT });
    await page.getByPlaceholder('Master Password').fill(MASTER_PASSWORD);
    await page.locator('#kp-keyfile-input').setInputFiles(KEYFILE);
    await page.getByRole('button', { name: 'Open Vault' }).click();
    await page.waitForURL(/\/keepass/, { timeout: LOGIN_TIMEOUT });
  }

  test('refuses a save that would drop the key file', async ({ page }) => {
    await unlock(page);

    // ask to save with the master password and no key file
    const refused = await page.evaluate(async () => {
      const response = await fetch('api/v1/save_db', {
        method: 'POST',
        headers: {
          'X-CSRF-Token': localStorage.getItem('CSRFToken'),
          'Content-Type': 'application/x-www-form-urlencoded',
        },
        body: 'password=test',
      });
      return response.status;
    });

    expect(refused).toBe(401);

    // the database is untouched: still needs the key file, still opens with it
    expect(opens(false)).toBe(false);
    expect(opens(true)).toBe(true);
  });

  test('refuses a save with a master password that is not the current one', async ({ page }) => {
    await unlock(page);

    const refused = await page.evaluate(async () => {
      const response = await fetch('api/v1/save_db', {
        method: 'POST',
        headers: {
          'X-CSRF-Token': localStorage.getItem('CSRFToken'),
          'Content-Type': 'application/x-www-form-urlencoded',
        },
        body: 'password=not-the-master-password',
      });
      return response.status;
    });

    expect(refused).toBe(401);
    expect(opens(true)).toBe(true);
  });

  // A database that is not there cannot vouch for any credentials. Creating
  // one to check against would accept all of them, so the save is refused.
  test('refuses a save when the stored database has gone missing', async ({ page }) => {
    await unlock(page);
    fs.rmSync(DB);

    const refused = await page.evaluate(async () => {
      const response = await fetch('api/v1/save_db', {
        method: 'POST',
        headers: {
          'X-CSRF-Token': localStorage.getItem('CSRFToken'),
          'Content-Type': 'application/x-www-form-urlencoded',
        },
        body: 'password=not-the-master-password',
      });
      return response.status;
    });

    expect(refused).toBe(401);
    // and nothing was written in its place
    expect(fs.existsSync(DB)).toBe(false);
  });

  test('saves when the key file is given again', async ({ page }) => {
    await unlock(page);

    await page.locator('[data-testid="tree-node"]').filter({ hasText: 'group1' }).click();
    await page.getByRole('button', { name: 'New Entry' }).click();
    await page.locator('#kp-f-title').fill('written with a key file');
    await page.getByRole('button', { name: 'Save Entry' }).click();

    await page.locator('.kp-nav-actions .kp-btn-primary').click();
    await page.locator('.kp-modal input[type="password"]').fill(MASTER_PASSWORD);
    // the form asks for the key file because the vault was opened with one
    await page.locator('[data-testid="save-keyfile"]').setInputFiles(KEYFILE);

    const saved = page.waitForResponse((r) => r.url().includes('/api/v1/save_db'));
    await page.locator('.kp-modal button:has-text("Save")').click();
    expect((await saved).status()).toBe(200);

    // written, and still guarded by both credentials
    expect(kxc(['ls', '-q', '-k', KEYFILE], ['group1'])).toContain('written with a key file');
    expect(opens(false)).toBe(false);
  });
});
