// Reorganising a vault: moving entries and groups around, deleting groups, and
// the entry details that were previously not shown at all.

const { test, expect } = require('@playwright/test');
const {
  DB, byId, entryRow, entryRows, fieldValue, groupTitle,
  openDb, openEntry, selectGroup, treeNode, treeRoot,
} = require('./helpers');

const picker = (page) => byId(page, 'group-picker');
// a row carries its group name and, for the one the item already sits in, the
// word 'current', so the name is matched as a prefix rather than exactly
const pickerRow = (page, title) => byId(page, 'group-picker-row')
  .filter({ hasText: new RegExp(`^${title}`) });

test.describe('Reorganising the vault', () => {
  test.beforeEach(async ({ page }) => {
    await openDb(page);
  });

  test('moves an entry to another group', async ({ page }) => {
    await selectGroup(page, 'group1');
    await entryRow(page, DB.entry.title).locator('[data-testid="move-entry"]').click();

    await expect(picker(page)).toBeVisible();
    // the group it already sits in is not a destination
    await expect(pickerRow(page, 'group1')).toBeDisabled();

    await pickerRow(page, 'group2').click();
    await expect(entryRow(page, DB.entry.title)).toHaveCount(0);

    await selectGroup(page, 'group2');
    await expect(entryRow(page, DB.entry.title)).toBeVisible();
  });

  test('moves a group under another group', async ({ page }) => {
    await selectGroup(page, 'group2');
    await byId(page, 'move-group').click();

    await expect(picker(page)).toBeVisible();
    await pickerRow(page, 'group1').click();

    // group2 is now a child of group1 rather than of the root
    await expect(treeNode(page, 'group2')).toBeVisible();
    await selectGroup(page, 'group1');
    await expect(byId(page, 'move-group')).toBeVisible();
  });

  test('will not offer a group its own descendant as a destination', async ({ page }) => {
    // put group2 under group1 first, so group1 has a descendant
    await selectGroup(page, 'group2');
    await byId(page, 'move-group').click();
    await pickerRow(page, 'group1').click();
    await expect(picker(page)).toHaveCount(0);

    await selectGroup(page, 'group1');
    await byId(page, 'move-group').click();

    await expect(picker(page)).toBeVisible();
    await expect(pickerRow(page, 'group1')).toHaveCount(0);
    await expect(pickerRow(page, 'group2')).toHaveCount(0);
    await expect(pickerRow(page, DB.root)).toBeVisible();
  });

  test('deletes a group into the recycle bin, entries and all', async ({ page }) => {
    await selectGroup(page, 'group1');
    await expect(entryRows(page)).toHaveCount(2);

    page.on('dialog', (dialog) => dialog.accept());
    await byId(page, 'delete-group').click();

    // the selection falls back to the root, and the group is in the bin with
    // the entries it held rather than gone
    await expect(groupTitle(page)).toHaveText(DB.root);
    await selectGroup(page, 'group1');
    await expect(entryRows(page)).toHaveCount(2);
    await expect(treeNode(page, 'Recycle Bin')).toBeVisible();
  });

  test('offers no delete or move on the root group', async ({ page }) => {
    await treeRoot(page).click();
    await expect(groupTitle(page)).toHaveText(DB.root);

    await expect(byId(page, 'delete-group')).toHaveCount(0);
    await expect(byId(page, 'move-group')).toHaveCount(0);
  });

  test('sends a deleted entry to the recycle bin rather than dropping it', async ({ page }) => {
    await selectGroup(page, 'group1');

    page.on('dialog', (dialog) => dialog.accept());
    await entryRow(page, DB.entry.clone).locator('[title="Delete entry"]').click();
    await expect(entryRow(page, DB.entry.clone)).toHaveCount(0);

    // the bin appears in the tree, holding the entry
    await selectGroup(page, 'Recycle Bin');
    await expect(entryRow(page, DB.entry.clone)).toBeVisible();
  });
});

test.describe('Entry details', () => {
  test.beforeEach(async ({ page }) => {
    await openDb(page);
    await selectGroup(page, 'group1');
  });

  test('marks an entry that is past its expiry', async ({ page }) => {
    // the fixture entries expired in 2023
    await openEntry(page, DB.entry.title);

    await expect(byId(page, 'entry-expired')).toBeVisible();
    await expect(byId(page, 'entry-expiry')).toContainText('2023');
  });

  test('shows when the entry was created and last changed', async ({ page }) => {
    await openEntry(page, DB.entry.title);

    await expect(byId(page, 'entry-created')).not.toHaveText('—');
    await expect(byId(page, 'entry-modified')).not.toHaveText('—');
  });

  test('names the files attached to an entry', async ({ page }) => {
    await openEntry(page, DB.entry.title);

    await expect(fieldValue(page, 'File')).toHaveText('favicon.ico.jpeg');
  });

  test('keeps a previous version when an entry is edited', async ({ page }) => {
    await openEntry(page, DB.entry.title);
    const before = await byId(page, 'entry-history').count();

    await page.getByRole('button', { name: 'Edit', exact: true }).click();
    await page.locator('#kp-f-username').fill('changed');
    await page.getByRole('button', { name: 'Save Entry' }).click();

    await openEntry(page, DB.entry.title);
    await expect(byId(page, 'entry-history')).toHaveCount(before + 1);
    // the version kept on top is the one from just before the edit
    await expect(byId(page, 'entry-history').first()).toContainText(DB.entry.username);
  });

  test('sets an expiry date on an entry', async ({ page }) => {
    await page.getByRole('button', { name: 'New Entry' }).click();
    await page.locator('#kp-f-title').fill('expires soon');

    // the date input stays disabled until the entry is marked as expiring
    await expect(page.locator('#kp-f-expiry')).toBeDisabled();
    await byId(page, 'entry-expires').check();
    await page.locator('#kp-f-expiry').fill('2020-01-02T03:04');

    await page.getByRole('button', { name: 'Save Entry' }).click();
    await openEntry(page, 'expires soon');

    await expect(byId(page, 'entry-expired')).toBeVisible();
    await expect(byId(page, 'entry-expiry')).toContainText('2020');
  });
});
