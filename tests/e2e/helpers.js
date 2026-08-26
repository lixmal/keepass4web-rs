const { expect } = require('@playwright/test');

// Opening the database runs the master password through the KDF, which takes
// several seconds in a debug build, so login steps get their own timeout.
const LOGIN_TIMEOUT = 45000;

const MASTER_PASSWORD = 'test';

// Contents of tests/test.kdbx, kept here so the specs read as behaviour rather
// than as a pile of literals.
const DB = {
  root: 'Root',
  groups: ['group1', 'group2'],
  entry: {
    title: 'entry1',
    clone: 'entry1 - Clone',
    username: 'someusr',
    password: 'somepass123',
    url: 'someurl',
    notes: 'somenote',
    tag: 'sometag',
    field: 'somesecretstring',
    fieldValue: 'secret',
  },
  masked: '••••••••',
};

// The vault is addressed through data-testid hooks rather than through its
// classes: the classes carry the styling and get reshuffled by design work,
// the hooks only exist for these tests.
const byId = (page, id) => page.locator(`[data-testid="${id}"]`);

const exactly = (text) => new RegExp(`^${text.replace(/[.*+?^${}()|[\]\\-]/g, '\\$&')}$`);

// '/' bounces through the splash and the password-less user login before the
// master password form appears.
async function gotoLogin(page) {
  await page.goto('/');
  await page.waitForURL(/\/db_login/, { timeout: LOGIN_TIMEOUT });
  await expect(page.getByRole('heading', { name: 'Open Vault' })).toBeVisible();
}

async function openDb(page, password = MASTER_PASSWORD) {
  await gotoLogin(page);
  await page.getByPlaceholder('Master Password').fill(password);
  await page.getByRole('button', { name: 'Open Vault' }).click();
  await page.waitForURL(/\/keepass/, { timeout: LOGIN_TIMEOUT });
  await expect(treeNode(page, DB.groups[0])).toBeVisible();
}

const treeRoot = (page) => byId(page, 'tree-root');

const treeNodes = (page) => byId(page, 'tree-node');

function treeNode(page, title) {
  return treeNodes(page).filter({ hasText: exactly(title) });
}

const groupTitle = (page) => byId(page, 'group-title');

async function selectGroup(page, title) {
  await treeNode(page, title).click();
  await expect(groupTitle(page)).toHaveText(title);
}

function entryRows(page) {
  return byId(page, 'entry-card');
}

function entryRow(page, title) {
  return entryRows(page).filter({
    has: page.locator('[data-testid="entry-card-title"]').filter({ hasText: exactly(title) }),
  });
}

// The title of the entry shown in the detail panel, absent while no entry is open.
const entryTitle = (page) => byId(page, 'entry-title');

async function openEntry(page, title) {
  await entryRow(page, title).click();
  await expect(entryTitle(page)).toHaveText(title);
}

// A field of the open entry, addressed by its label ('Password', 'Notes', or
// the name of a custom field).
function fieldRow(page, label) {
  return byId(page, 'entry-field').filter({
    has: page.locator('[data-testid="entry-field-label"]').filter({ hasText: exactly(label) }),
  });
}

const fieldValue = (page, label) => fieldRow(page, label).locator('[data-testid="entry-field-value"]');
const revealButton = (page, label) => fieldRow(page, label).locator('[data-testid="reveal"]');
const copyButton = (page, label) => fieldRow(page, label).locator('[data-testid="copy"]');

// Protected fields carry their reveal state, so the specs can assert on it
// without reaching into the icon that happens to be drawn.
const expectRevealed = (page, label, revealed) =>
  expect(revealButton(page, label)).toHaveAttribute('data-revealed', String(revealed));

const clipboard = (page) => page.evaluate(() => navigator.clipboard.readText());

async function expectClipboard(page, value) {
  await expect.poll(() => clipboard(page), { timeout: 10000 }).toBe(value);
}

module.exports = {
  DB,
  LOGIN_TIMEOUT,
  MASTER_PASSWORD,
  byId,
  clipboard,
  copyButton,
  entryRow,
  entryRows,
  entryTitle,
  exactly,
  expectClipboard,
  expectRevealed,
  fieldRow,
  fieldValue,
  gotoLogin,
  groupTitle,
  openDb,
  openEntry,
  revealButton,
  selectGroup,
  treeNode,
  treeNodes,
  treeRoot,
};
