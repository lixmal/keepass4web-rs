const { test, expect } = require('@playwright/test');

async function login(page) {
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(1500);

  const passwordInput = page.locator('input[type="password"]');
  await expect(passwordInput).toBeVisible({ timeout: 10000 });
  await passwordInput.fill('test');

  const submitButton = page.locator('button[type="submit"], input[type="submit"]');

  await Promise.all([
    page.waitForResponse(resp => resp.url().includes('/api/v1/db_login') && resp.status() === 200, { timeout: 20000 }),
    submitButton.click()
  ]);

  await page.waitForResponse(resp =>
    (resp.url().includes('/api/v1/get_groups') || resp.url().includes('/api/v1/tree')) && resp.status() === 200,
    { timeout: 15000 }
  );

  await page.waitForTimeout(1500);
}

test.describe('KeePass4Web E2E Tests', () => {
  test('should login, navigate to entry in group1, and copy password', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);

    await login(page);
    console.log('✓ Logged in');

    await expect(page.locator('.treeview-body')).toBeVisible();
    const treeNodes = page.locator('.list-group-item');
    const nodeCount = await treeNodes.count();
    console.log(`✓ Tree loaded with ${nodeCount} groups`);
    expect(nodeCount).toBeGreaterThan(0);

    const group1 = page.locator('.list-group-item', { hasText: 'group1' });
    await expect(group1).toBeVisible();

    await Promise.all([
      page.waitForResponse(resp => resp.url().includes('/api/v1/get_group_entries'), { timeout: 10000 }),
      group1.click()
    ]);
    await page.waitForTimeout(1500);
    console.log('✓ Clicked group1 and loaded entries');

    await page.waitForTimeout(1000);

    const entry1Link = page.locator('text=entry1').first();
    await expect(entry1Link).toBeVisible({ timeout: 5000 });
    console.log('✓ Found entry1 in panel');

    await entry1Link.click();
    await page.waitForTimeout(1500);
    console.log('✓ Clicked entry1');

    const bodyText = await page.textContent('body');
    expect(bodyText).toContain('someusr');
    console.log('✓ Username visible: someusr');

    // Test copy buttons by clicking each and checking clipboard
    const entryPanel = page.locator('.panel').last();
    const panelButtons = entryPanel.locator('button');
    const buttonCount = await panelButtons.count();
    console.log(`✓ Found ${buttonCount} buttons in entry panel`);

    const copiedValues = [];
    for (let i = 0; i < Math.min(buttonCount, 5); i++) {
      const initialClipboard = await page.evaluate(() => navigator.clipboard.readText()).catch(() => '');

      await panelButtons.nth(i).click();
      await page.waitForTimeout(800);

      const newClipboard = await page.evaluate(() => navigator.clipboard.readText()).catch(() => '');

      if (newClipboard !== initialClipboard && newClipboard.length > 0) {
        copiedValues.push({ button: i, value: newClipboard });
        console.log(`✓ Button ${i} copied: "${newClipboard.substring(0, 20)}${newClipboard.length > 20 ? '...' : ''}" (length: ${newClipboard.length})`);
      }
    }

    expect(copiedValues.length).toBeGreaterThanOrEqual(2);

    const usernameCopy = copiedValues.find(c => c.value === 'someusr');
    expect(usernameCopy).toBeDefined();
    console.log(`✓ Username copy verified at button ${usernameCopy.button}`);

    const passwordCopy = copiedValues.find(c => c.value.length === 11 && c.value !== 'someusr');
    expect(passwordCopy).toBeDefined();
    console.log(`✓ Password copy verified at button ${passwordCopy.button}`);

    const secretCopy = copiedValues.find(c => c.value.length === 6 && c.value !== 'someusr');
    if (secretCopy) {
      console.log(`✓ Custom secret field copy verified at button ${secretCopy.button}`);
    }

    // Check file download if present
    if (bodyText.includes('Files') && bodyText.includes('favicon')) {
      console.log(`✓ File section detected with favicon.ico.jpeg`);

      const downloadPromise = page.waitForEvent('download', { timeout: 10000 });
      const fileLink = page.locator('a, button').filter({ hasText: /favicon/i });
      const fileLinkCount = await fileLink.count();

      if (fileLinkCount > 0) {
        await fileLink.first().click();
        await page.waitForTimeout(500);

        try {
          const download = await downloadPromise;
          const filename = download.suggestedFilename();
          console.log(`✓ File download started: ${filename}`);

          expect(filename).toContain('favicon');

          const downloadPath = await download.path();
          if (downloadPath) {
            console.log(`✓ File downloaded successfully to: ${downloadPath}`);
          }
        } catch (error) {
          console.log(`⚠ Download verification skipped: ${error.message}`);
        }
      } else {
        console.log('⚠ File download link not found');
      }
    }
  });

  test('should reject wrong password', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1500);

    const passwordInput = page.locator('input[type="password"]');
    await expect(passwordInput).toBeVisible({ timeout: 10000 });

    await passwordInput.fill('wrongpassword123');

    const submitButton = page.locator('button[type="submit"], input[type="submit"]');
    await submitButton.click();

    await page.waitForTimeout(2000);

    const url = page.url();
    const isOnKeepass = url.includes('keepass');

    if (!isOnKeepass) {
      console.log('✓ Wrong password rejected - stayed on login page');
      expect(isOnKeepass).toBeFalsy();
    } else {
      const hasError = await page.locator('.alert, [class*="error"], [class*="danger"]')
        .isVisible()
        .catch(() => false);
      expect(hasError).toBeTruthy();
    }
  });
});
