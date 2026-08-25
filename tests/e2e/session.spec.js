const { test, expect } = require('@playwright/test');
const { LOGIN_TIMEOUT, openDb, selectGroup, treeNode } = require('./helpers');

const timer = (page) => page.locator('.navbar-text span').first();

// The user menu is a bootstrap dropdown, closed until its toggle is clicked.
async function openUserMenu(page) {
  await page.locator('.dropdown-toggle').click();
  await expect(page.locator('.dropdown-menu')).toBeVisible();
}

test.describe('The database session', () => {
  test.beforeEach(async ({ page }) => {
    await openDb(page);
  });

  test('counts down the time left until the database closes', async ({ page }) => {
    // tests/config.test.yml closes the database after five minutes
    await expect(timer(page)).toHaveText(/^00:0[45]:\d{2}$/);

    const before = await timer(page).textContent();
    await expect.poll(() => timer(page).textContent(), { timeout: 5000 })
      .not.toBe(before);
  });

  test('restarts the countdown when the reset button is clicked', async ({ page }) => {
    await expect.poll(() => timer(page).textContent(), { timeout: 10000 })
      .toMatch(/^00:04:5[0-6]$/);

    await page.locator('.navbar-text label').click();

    await expect.poll(() => timer(page).textContent(), { timeout: 5000 })
      .toMatch(/^00:0[45]:(00|59|58)$/);
  });

  test('closes the database from the user menu', async ({ page }) => {
    await openUserMenu(page);
    await page.locator('#closeDB').click();

    await page.waitForURL(/\/db_login/, { timeout: LOGIN_TIMEOUT });
    await expect(page.getByPlaceholder('Master Password')).toBeVisible();
  });

  test('asks for the master password again after the database was closed', async ({ page }) => {
    await openUserMenu(page);
    await page.locator('#closeDB').click();
    await page.waitForURL(/\/db_login/, { timeout: LOGIN_TIMEOUT });

    await page.getByPlaceholder('Master Password').fill('test');
    await page.getByRole('button', { name: 'Open' }).click();

    await page.waitForURL(/\/keepass/, { timeout: LOGIN_TIMEOUT });
    await expect(treeNode(page, 'group1')).toBeVisible();
  });

  test('drops the session on logout', async ({ page }) => {
    const before = await page.evaluate(() => localStorage.getItem('CSRFToken'));

    await openUserMenu(page);
    await page.locator('#logout').click();

    await page.waitForURL(/\/db_login/, { timeout: LOGIN_TIMEOUT });
    // the test config logs users in without a password, so a new session is
    // handed out right away: it has to be a different one
    const after = await page.evaluate(() => localStorage.getItem('CSRFToken'));
    expect(after).not.toBe(before);
  });

  test('reports the database as closed once it was closed', async ({ page }) => {
    await selectGroup(page, 'group1');

    await openUserMenu(page);
    await page.locator('#closeDB').click();
    await page.waitForURL(/\/db_login/, { timeout: LOGIN_TIMEOUT });

    const session = await page.evaluate(async () => {
      const response = await fetch('api/v1/authenticated', {
        headers: { 'X-CSRF-Token': localStorage.getItem('CSRFToken') },
      });
      return { status: response.status, body: await response.json() };
    });
    expect(session.status).toBe(401);
    expect(session.body.data.db).toBe(false);
  });

  test('refuses an API request without the CSRF token', async ({ page }) => {
    const status = await page.evaluate(async () => {
      const response = await fetch('api/v1/get_groups');
      return response.status;
    });

    expect(status).toBe(403);
  });
});
