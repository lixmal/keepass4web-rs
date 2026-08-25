const { test, expect } = require('@playwright/test');
const { CARD_HEADER, DB, entryRow, entryRows, openDb, openEntry, selectGroup } = require('./helpers');

async function search(page, term) {
  await page.getByPlaceholder('Search').fill(term);
  await page.getByPlaceholder('Search').press('Enter');
  await expect(page.locator(`#group-viewer ${CARD_HEADER}`)).toHaveText(`Search results for '${term.trim()}'`);
}

test.describe('Searching entries', () => {
  test.beforeEach(async ({ page }) => {
    await openDb(page);
  });

  test('finds an entry by title', async ({ page }) => {
    await search(page, DB.entry.title);

    await expect(entryRows(page)).toHaveCount(2);
    await expect(entryRow(page, DB.entry.title)).toBeVisible();
    await expect(entryRow(page, DB.entry.clone)).toBeVisible();
  });

  // the configured search fields in tests/config.test.yml
  for (const [field, term] of Object.entries({
    username: DB.entry.username,
    tags: DB.entry.tag,
    notes: DB.entry.notes,
    url: DB.entry.url,
  })) {
    test(`finds an entry by ${field}`, async ({ page }) => {
      await search(page, term);

      await expect(entryRow(page, DB.entry.title)).toBeVisible();
    });
  }

  // extra_fields is on in tests/config.test.yml, so custom strings are searched too
  test('finds an entry by the value of a custom field', async ({ page }) => {
    await search(page, DB.entry.fieldValue);

    await expect(entryRow(page, DB.entry.title)).toBeVisible();
  });

  test('reports no results for a term that matches nothing', async ({ page }) => {
    await search(page, 'nothing matches this');

    await expect(entryRows(page)).toHaveCount(0);
  });

  test('trims whitespace around the term', async ({ page }) => {
    await search(page, `  ${DB.entry.title}  `);

    await expect(entryRows(page)).toHaveCount(2);
  });

  test('does not leak the protected fields of a match', async ({ page }) => {
    await search(page, DB.entry.password);

    await expect(entryRows(page)).toHaveCount(0);
  });

  test('closes the open entry when a search starts', async ({ page }) => {
    await selectGroup(page, 'group1');
    await openEntry(page, DB.entry.title);

    await search(page, DB.entry.title);

    await expect(page.locator(`#node-viewer ${CARD_HEADER}`)).toHaveCount(0);
  });

  test('opens an entry from the search results', async ({ page }) => {
    await search(page, DB.entry.clone);

    await openEntry(page, DB.entry.clone);

    await expect(page.locator('#node-viewer td', { hasText: DB.entry.username })).toBeVisible();
  });
});
