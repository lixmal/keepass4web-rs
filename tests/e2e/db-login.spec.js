const { test, expect } = require('@playwright/test');
const { DB, LOGIN_TIMEOUT, gotoLogin, openDb, treeNode, treeNodes, treeRoot } = require('./helpers');

test.describe('Opening the database', () => {
  test('sends an unauthenticated visitor to the master password form', async ({ page }) => {
    await gotoLogin(page);

    await expect(page).toHaveURL(/\/db_login$/);
    await expect(page.getByPlaceholder('Master Password')).toBeFocused();
    await expect(page.getByRole('button', { name: 'Open Vault' })).toBeVisible();
  });

  test('sends a visitor asking for /keepass to the login form as well', async ({ page }) => {
    await page.goto('/keepass');

    await page.waitForURL(/\/db_login/, { timeout: LOGIN_TIMEOUT });
  });

  test('opens the database and shows the group tree', async ({ page }) => {
    await openDb(page);

    await expect(page).toHaveURL(/\/keepass$/);
    await expect(treeRoot(page)).toHaveText(DB.root);
    await expect(treeNodes(page)).toHaveCount(DB.groups.length);
  });

  test('stores the CSRF token and the session settings for later requests', async ({ page }) => {
    await openDb(page);

    const stored = await page.evaluate(() => ({
      token: localStorage.getItem('CSRFToken'),
      settings: JSON.parse(localStorage.getItem('settings')),
    }));

    expect(stored.token).toMatch(/^\w+$/);
    expect(stored.settings.timeout).toBeGreaterThan(0);
  });

  test('keeps the database open across a page reload', async ({ page }) => {
    await openDb(page);

    await page.reload();

    await page.waitForURL(/\/keepass$/, { timeout: LOGIN_TIMEOUT });
    await expect(treeNode(page, DB.groups[0])).toBeVisible();
  });

  test('rejects a wrong master password and stays on the login form', async ({ page }) => {
    await gotoLogin(page);
    await page.getByPlaceholder('Master Password').fill('not the master password');
    await page.getByRole('button', { name: 'Open Vault' }).click();

    await expect(page.locator('.login-error')).toBeVisible({ timeout: LOGIN_TIMEOUT });
    await expect(page).toHaveURL(/\/db_login/);
    await expect(treeNodes(page)).toHaveCount(0);
  });

  test('rejects an empty master password', async ({ page }) => {
    await gotoLogin(page);
    await page.getByRole('button', { name: 'Open Vault' }).click();

    await expect(page.locator('.login-error')).toBeVisible({ timeout: LOGIN_TIMEOUT });
    await expect(page).toHaveURL(/\/db_login/);
  });

  test('opens the database after a failed attempt', async ({ page }) => {
    await gotoLogin(page);
    await page.getByPlaceholder('Master Password').fill('not the master password');
    await page.getByRole('button', { name: 'Open Vault' }).click();
    await expect(page.locator('.login-error')).toBeVisible({ timeout: LOGIN_TIMEOUT });

    await openDb(page);
  });
});
