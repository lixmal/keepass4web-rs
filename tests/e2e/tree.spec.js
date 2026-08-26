const { test, expect } = require('@playwright/test');
const {
  DB, entryRow, entryRows, entryTitle, groupTitle, openDb, selectGroup, treeNode, treeNodes, treeRoot,
} = require('./helpers');

test.describe('Browsing groups', () => {
  test.beforeEach(async ({ page }) => {
    await openDb(page);
  });

  test('lists the groups of the database under the root node', async ({ page }) => {
    await expect(treeRoot(page)).toHaveText(DB.root);
    await expect(treeNodes(page)).toHaveText(DB.groups);
  });

  test('shows the entries of a group with their usernames', async ({ page }) => {
    await selectGroup(page, 'group1');

    await expect(entryRows(page)).toHaveCount(2);
    const row = entryRow(page, DB.entry.title);
    await expect(row.locator('[data-testid="entry-card-title"]')).toHaveText(DB.entry.title);
    await expect(row.locator('[data-testid="entry-card-meta"]')).toContainText(DB.entry.username);
    await expect(entryRow(page, DB.entry.clone)).toBeVisible();
  });

  test('shows an empty group without entries', async ({ page }) => {
    await selectGroup(page, 'group2');

    await expect(entryRows(page)).toHaveCount(0);
  });

  test('switches the entry list when another group is picked', async ({ page }) => {
    await selectGroup(page, 'group1');
    await expect(entryRows(page)).toHaveCount(2);

    await selectGroup(page, 'group2');

    await expect(entryRows(page)).toHaveCount(0);
  });

  test('drops the open entry when the group changes', async ({ page }) => {
    await selectGroup(page, 'group1');
    await entryRow(page, DB.entry.title).click();
    await expect(entryTitle(page)).toHaveText(DB.entry.title);

    await selectGroup(page, 'group2');

    await expect(entryTitle(page)).toHaveCount(0);
  });

  test('serves the icons the tree and the entries point at', async ({ page }) => {
    await selectGroup(page, 'group1');
    await entryRow(page, DB.entry.title).click();

    const sources = await page.locator('img.kp-icon').evaluateAll(
      (images) => images.map((image) => image.src));
    expect(sources.length).toBeGreaterThan(0);

    for (const src of new Set(sources)) {
      const response = await page.request.get(src);
      expect(response.status(), src).toBe(200);
    }
  });

  test('selecting the root group shows it without entries', async ({ page }) => {
    await treeRoot(page).click();

    await expect(groupTitle(page)).toHaveText(DB.root);
    await expect(entryRows(page)).toHaveCount(0);
  });
});
