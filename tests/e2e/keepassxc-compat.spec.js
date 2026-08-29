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

  async function openVault(page) {
    await page.goto('/');
    await page.waitForURL(/\/db_login/, { timeout: LOGIN_TIMEOUT });
    await page.getByPlaceholder('Master Password').fill(MASTER_PASSWORD);
    await page.getByRole('button', { name: 'Open Vault' }).click();
    await page.waitForURL(/\/keepass/, { timeout: LOGIN_TIMEOUT });
  }

  test('keepassxc reads an entry this app wrote', async ({ page }) => {
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

  test('keeps attachments readable by keepassxc across a save', async ({ page }) => {
    // saving used to drop the reference an entry holds to its attachment,
    // leaving the bytes in the file with nothing pointing at them
    const before = path.join(path.dirname(DB), 'attachment-before');
    fs.rmSync(before, { force: true });
    kxc(['attachment-export', '-q'], ['group1/entry1', 'favicon.ico.jpeg', before]);
    const original = fs.readFileSync(before);
    expect(original.length).toBeGreaterThan(0);

    await openVault(page);

    await page.locator('[data-testid="tree-node"]').filter({ hasText: 'group1' }).click();
    await page.getByRole('button', { name: 'New Entry' }).click();
    await page.locator('#kp-f-title').fill('saved alongside an attachment');
    await page.getByRole('button', { name: 'Save Entry' }).click();

    await page.locator('.kp-nav-actions .kp-btn-primary').click();
    await page.locator('.kp-modal input[type="password"]').fill(MASTER_PASSWORD);
    const saved = page.waitForResponse((r) => r.url().includes('/api/v1/save_db'));
    await page.locator('.kp-modal button:has-text("Save")').click();
    expect((await saved).status()).toBe(200);

    // keepassxc still finds the attachment, byte for byte, next to the new entry
    const after = path.join(path.dirname(DB), 'attachment-after');
    fs.rmSync(after, { force: true });
    kxc(['attachment-export', '-q'], ['group1/entry1', 'favicon.ico.jpeg', after]);
    expect(fs.readFileSync(after).equals(original)).toBe(true);
    expect(kxc(['ls', '-q'], ['group1'])).toContain('saved alongside an attachment');
  });

  test('serves an attachment for download', async ({ page }) => {
    await openVault(page);

    await page.locator('[data-testid="tree-node"]').filter({ hasText: 'group1' }).click();
    await page.locator('[data-testid="entry-card"]').filter({ hasText: 'entry1' }).first().click();

    const entryId = await page.evaluate(async () => {
      const groups = await fetch('api/v1/get_groups', {
        headers: { 'X-CSRF-Token': localStorage.getItem('CSRFToken') },
      }).then((r) => r.json());
      const group1 = groups.data.groups.children.find((g) => g.title === 'group1');
      const entries = await fetch(`api/v1/get_group_entries?id=${group1.id}`, {
        headers: { 'X-CSRF-Token': localStorage.getItem('CSRFToken') },
      }).then((r) => r.json());
      return entries.data.entries.find((e) => e.title === 'entry1').id;
    });

    const downloaded = await page.evaluate(async (id) => {
      const response = await fetch(`api/v1/get_file?entry_id=${id}&filename=favicon.ico.jpeg`, {
        headers: { 'X-CSRF-Token': localStorage.getItem('CSRFToken') },
      });
      return { status: response.status, size: (await response.arrayBuffer()).byteLength };
    }, entryId);

    expect(downloaded.status).toBe(200);

    const exported = path.join(path.dirname(DB), 'exported-attachment');
    fs.rmSync(exported, { force: true });
    kxc(['attachment-export', '-q'], ['group1/entry1', 'favicon.ico.jpeg', exported]);
    expect(downloaded.size).toBe(fs.statSync(exported).size);
  });

  test('a deleted entry lands in the recycle bin keepassxc knows', async ({ page }) => {
    await openVault(page);

    await page.locator('[data-testid="tree-node"]').filter({ hasText: 'group1' }).click();
    const entry = page.locator('[data-testid="entry-card"]').filter({ hasText: 'entry1 - Clone' });
    await expect(entry).toBeVisible();

    page.on('dialog', (dialog) => dialog.accept());
    await entry.locator('[title="Delete entry"]').click();
    await expect(entry).toHaveCount(0);

    await page.locator('.kp-nav-actions .kp-btn-primary').click();
    await page.locator('.kp-modal input[type="password"]').fill(MASTER_PASSWORD);
    const saved = page.waitForResponse((r) => r.url().includes('/api/v1/save_db'));
    await page.locator('.kp-modal button:has-text("Save")').click();
    expect((await saved).status()).toBe(200);

    // deleting must not destroy the entry: keepassxc finds it in the bin
    expect(kxc(['ls', '-q'], ['group1'])).not.toContain('entry1 - Clone');
    expect(kxc(['ls', '-q'], ['Recycle Bin'])).toContain('entry1 - Clone');
  });

  test('emptying the recycle bin deletes for good', async ({ page }) => {
    // keepassxc puts it in the bin, the app deletes it from there permanently
    kxc(['rm', '-q'], ['group1/entry1 - Clone']);
    expect(kxc(['ls', '-q'], ['Recycle Bin'])).toContain('entry1 - Clone');

    await openVault(page);

    await page.locator('[data-testid="tree-node"]').filter({ hasText: 'Recycle Bin' }).click();
    const entry = page.locator('[data-testid="entry-card"]').filter({ hasText: 'entry1 - Clone' });
    await expect(entry).toBeVisible();

    page.on('dialog', (dialog) => dialog.accept());
    await entry.locator('[title="Delete entry"]').click();
    await expect(entry).toHaveCount(0);

    await page.locator('.kp-nav-actions .kp-btn-primary').click();
    await page.locator('.kp-modal input[type="password"]').fill(MASTER_PASSWORD);
    const saved = page.waitForResponse((r) => r.url().includes('/api/v1/save_db'));
    await page.locator('.kp-modal button:has-text("Save")').click();
    expect((await saved).status()).toBe(200);

    expect(kxc(['ls', '-q'], ['Recycle Bin'])).not.toContain('entry1 - Clone');
  });
});
