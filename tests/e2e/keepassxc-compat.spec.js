// A database this app writes has to stay a database other KeePass clients can
// read, and the other way round. These specs drive the app through the browser
// and check the file itself with keepassxc-cli.

const { execFileSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const { test, expect } = require('@playwright/test');
const { LOGIN_TIMEOUT, MASTER_PASSWORD } = require('./helpers');

const FIXTURE = path.join(__dirname, '..', 'test.kdbx');
const DB = path.join(__dirname, '..', 'tmp', 'compat.kdbx');

// keepassxc-cli takes the database before the entry path and reads the
// database password from stdin
function kxc(flags, positional = [], input = `${MASTER_PASSWORD}\n`) {
  return execFileSync('keepassxc-cli', [...flags, DB, ...positional], {
    input,
    encoding: 'utf8',
    stdio: ['pipe', 'pipe', 'pipe'],
  });
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

test.describe('KeePassXC compatibility', () => {
  test.skip(!installed, 'keepassxc-cli is not installed');

  test.beforeEach(async () => {
    // every spec starts from the untouched fixture, since these ones write
    fs.mkdirSync(path.dirname(DB), { recursive: true });
    fs.copyFileSync(FIXTURE, DB);
  });

  // The fixture entries carry an attachment, in their history as well, and
  // saving a database that has one is refused. The specs that write replace
  // those entries with fresh ones, which drops the history that holds the
  // attachment, and empty the recycle bin the removal fills. Creating a
  // database outright is not an option: keepassxc writes a newer kdbx version
  // than this app can save.
  function withoutAttachments() {
    const entries = ['entry1', 'entry1 - Clone'];

    for (const title of entries) {
      kxc(['rm', '-q'], [`group1/${title}`]);
    }
    for (const title of entries) {
      kxc(['rm', '-q'], [`Recycle Bin/${title}`]);
      kxc(['add', '-q', '-u', 'someusr', '--url', 'someurl', '-p'], [`group1/${title}`],
        `${MASTER_PASSWORD}\nsomepass123\n`);
    }
  }

  async function openVault(page) {
    await page.goto('/');
    await page.waitForURL(/\/db_login/, { timeout: LOGIN_TIMEOUT });
    await page.getByPlaceholder('Master Password').fill(MASTER_PASSWORD);
    await page.getByRole('button', { name: 'Open Vault' }).click();
    await page.waitForURL(/\/keepass/, { timeout: LOGIN_TIMEOUT });
  }

  test('keepassxc reads an entry this app wrote', async ({ page }) => {
    withoutAttachments();
    await openVault(page);

    await page.locator('[data-testid="tree-node"]').filter({ hasText: 'group1' }).click();
    await page.getByRole('button', { name: 'New Entry' }).click();
    await page.locator('#kp-f-title').fill('written here');
    await page.locator('#kp-f-username').fill('someone');
    await page.locator('#kp-f-password').fill('a-password-worth-keeping');
    await page.getByRole('button', { name: 'Save Entry' }).click();
    await expect(page.locator('[data-testid="entry-card"]').filter({ hasText: 'written here' }))
      .toBeVisible();

    await page.locator('.kp-nav-actions .kp-btn-primary').click();
    await page.locator('.kp-modal input[type="password"]').fill(MASTER_PASSWORD);
    const saved = page.waitForResponse((r) => r.url().includes('/api/v1/save_db'));
    await page.locator('.kp-modal button:has-text("Save")').click();
    expect((await saved).status()).toBe(200);
    await expect(page.locator('.kp-modal')).toHaveCount(0);

    const shown = kxc(['show', '-q', '-s'], ['group1/written here']);
    expect(shown).toContain('UserName: someone');
    expect(shown).toContain('Password: a-password-worth-keeping');

    // the entries that were already there survived the rewrite
    const listed = kxc(['ls', '-q'], ['group1']);
    expect(listed).toContain('entry1');
    expect(listed).toContain('entry1 - Clone');
  });

  test('this app reads an entry keepassxc wrote', async ({ page }) => {
    kxc(['add', '-q', '-u', 'kxcuser', '-p'], ['group2/from keepassxc'],
      `${MASTER_PASSWORD}\nkxcpass456\n`);

    await openVault(page);
    await page.locator('[data-testid="tree-node"]').filter({ hasText: 'group2' }).click();

    const entry = page.locator('[data-testid="entry-card"]').filter({ hasText: 'from keepassxc' });
    await expect(entry).toBeVisible();
    await entry.click();
    await expect(page.locator('[data-testid="entry-field-value"]').first()).toHaveText('kxcuser');
  });

  test('saving keeps the key derivation the database was created with', async ({ page }) => {
    withoutAttachments();
    const before = kxc(['db-info', '-q']);

    await openVault(page);
    await page.locator('[data-testid="tree-node"]').filter({ hasText: 'group1' }).click();
    await page.getByRole('button', { name: 'New Entry' }).click();
    await page.locator('#kp-f-title').fill('kdf check');
    await page.getByRole('button', { name: 'Save Entry' }).click();
    await page.locator('.kp-nav-actions .kp-btn-primary').click();
    await page.locator('.kp-modal input[type="password"]').fill(MASTER_PASSWORD);
    const saved = page.waitForResponse((r) => r.url().includes('/api/v1/save_db'));
    await page.locator('.kp-modal button:has-text("Save")').click();
    expect((await saved).status()).toBe(200);
    await expect(page.locator('.kp-modal')).toHaveCount(0);

    const after = kxc(['db-info', '-q']);
    const field = (info, name) => info.split('\n').find((line) => line.startsWith(`${name}:`));

    // a weaker kdf or cipher after a save would be a silent downgrade
    expect(field(after, 'KDF')).toBe(field(before, 'KDF'));
    expect(field(after, 'Cipher')).toBe(field(before, 'Cipher'));
  });

  test('keepassxc reads the tags and custom fields this app wrote', async ({ page }) => {
    withoutAttachments();
    await openVault(page);

    await page.locator('[data-testid="tree-node"]').filter({ hasText: 'group1' }).click();
    await page.getByRole('button', { name: 'New Entry' }).click();
    await page.locator('#kp-f-title').fill('every field');
    await page.locator('#kp-f-username').fill('fielduser');
    await page.locator('#kp-f-password').fill('fieldpass');
    await page.locator('#kp-f-url').fill('https://example.org');
    await page.locator('#kp-f-notes').fill('a note worth keeping');
    await page.locator('#kp-f-tags').fill('alpha, beta');

    await page.locator('[data-testid="custom-field-add"]').click();
    await page.locator('[data-testid="custom-field-name"]').last().fill('plainfield');
    await page.locator('[data-testid="custom-field-value"]').last().fill('plainvalue');

    await page.locator('[data-testid="custom-field-add"]').click();
    await page.locator('[data-testid="custom-field-name"]').last().fill('secretfield');
    await page.locator('[data-testid="custom-field-value"]').last().fill('secretvalue');
    await page.locator('[data-testid="custom-field-protected"]').last().check();

    await page.getByRole('button', { name: 'Save Entry' }).click();
    await expect(page.locator('[data-testid="entry-card"]').filter({ hasText: 'every field' }))
      .toBeVisible();

    await page.locator('.kp-nav-actions .kp-btn-primary').click();
    await page.locator('.kp-modal input[type="password"]').fill(MASTER_PASSWORD);
    const saved = page.waitForResponse((r) => r.url().includes('/api/v1/save_db'));
    await page.locator('.kp-modal button:has-text("Save")').click();
    expect((await saved).status()).toBe(200);

    const shown = kxc(['show', '-q', '-s', '--all'], ['group1/every field']);
    expect(shown).toContain('UserName: fielduser');
    expect(shown).toContain('Password: fieldpass');
    expect(shown).toContain('URL: https://example.org');
    expect(shown).toContain('Notes: a note worth keeping');
    expect(shown).toContain('Tags: alpha,beta');
    expect(shown).toContain('plainfield: plainvalue');
    expect(shown).toContain('secretfield: secretvalue');
  });

  test('refuses to save a database whose attachments it would orphan', async ({ page }) => {
    // the fixture entries carry one, and the reference to it does not survive
    // a write, so the database is left alone instead
    await openVault(page);

    await page.locator('[data-testid="tree-node"]').filter({ hasText: 'group1' }).click();
    await page.getByRole('button', { name: 'New Entry' }).click();
    await page.locator('#kp-f-title').fill('not saved');
    await page.getByRole('button', { name: 'Save Entry' }).click();

    await page.locator('.kp-nav-actions .kp-btn-primary').click();
    await page.locator('.kp-modal input[type="password"]').fill(MASTER_PASSWORD);
    const saved = page.waitForResponse((r) => r.url().includes('/api/v1/save_db'));
    await page.locator('.kp-modal button:has-text("Save")').click();
    expect((await saved).status()).toBe(409);

    // the attachment and the entries around it are still there
    const exported = path.join(path.dirname(DB), 'exported-attachment');
    fs.rmSync(exported, { force: true });
    kxc(['attachment-export', '-q'], ['group1/entry1', 'favicon.ico.jpeg', exported]);
    expect(fs.statSync(exported).size).toBeGreaterThan(0);
    expect(kxc(['ls', '-q'], ['group1'])).not.toContain('not saved');
  });
});
