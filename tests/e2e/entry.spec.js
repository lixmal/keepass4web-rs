const { test, expect } = require('@playwright/test');
const {
  DB,
  copyButton,
  expectClipboard,
  fieldRow,
  fieldValue,
  ICON_HIDDEN,
  ICON_SHOWN,
  openDb,
  openEntry,
  revealButton,
  selectGroup,
} = require('./helpers');

test.describe('Viewing an entry', () => {
  test.beforeEach(async ({ page }) => {
    await openDb(page);
    await selectGroup(page, 'group1');
    await openEntry(page, DB.entry.title);
  });

  test('shows the unprotected fields of the entry', async ({ page }) => {
    await expect(fieldValue(page, 'Username')).toHaveText(DB.entry.username);
    await expect(fieldValue(page, 'Notes')).toHaveText(DB.entry.notes);
    await expect(fieldRow(page, 'URL').getByRole('link')).toHaveAttribute('href', DB.entry.url);
    await expect(fieldRow(page, 'Tags').locator('.badge')).toHaveText(DB.entry.tag);
  });

  test('keeps the password and the custom field masked until asked', async ({ page }) => {
    await expect(fieldValue(page, 'Password')).toHaveText(DB.masked);
    await expect(fieldValue(page, DB.entry.field)).toHaveText(DB.masked);
  });

  test('reveals the password and hides it again', async ({ page }) => {
    await revealButton(page, 'Password').click();

    await expect(fieldValue(page, 'Password')).toHaveText(DB.entry.password);
    await expect(revealButton(page, 'Password').locator('span')).toHaveClass(ICON_SHOWN);

    await revealButton(page, 'Password').click();

    await expect(fieldValue(page, 'Password')).toHaveText(DB.masked);
    await expect(revealButton(page, 'Password').locator('span')).toHaveClass(ICON_HIDDEN);
  });

  test('reveals a protected custom field on its own', async ({ page }) => {
    await revealButton(page, DB.entry.field).click();

    await expect(fieldValue(page, DB.entry.field)).toHaveText(DB.entry.fieldValue);
    // revealing one protected field leaves the others alone
    await expect(fieldValue(page, 'Password')).toHaveText(DB.masked);
  });

  test('copies the username', async ({ page }) => {
    await copyButton(page, 'Username').click();

    await expectClipboard(page, DB.entry.username);
  });

  test('copies the password without showing it', async ({ page }) => {
    await copyButton(page, 'Password').click();

    await expectClipboard(page, DB.entry.password);
    await expect(fieldValue(page, 'Password')).toHaveText(DB.masked);
  });

  test('copies a protected custom field', async ({ page }) => {
    await copyButton(page, DB.entry.field).click();

    await expectClipboard(page, DB.entry.fieldValue);
    await expect(fieldValue(page, DB.entry.field)).toHaveText(DB.masked);
  });

  test('re-masks a revealed password when another entry is opened', async ({ page }) => {
    await revealButton(page, 'Password').click();
    await expect(fieldValue(page, 'Password')).toHaveText(DB.entry.password);

    await openEntry(page, DB.entry.clone);

    await expect(fieldValue(page, 'Password')).toHaveText(DB.masked);
    await expect(revealButton(page, 'Password').locator('span')).toHaveClass(ICON_HIDDEN);
  });
});
