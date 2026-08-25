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
  masked: '******',
};

// Bootstrap renamed panels to cards in v5 and swapped glyphicons for its own
// icon font: accept both spellings so the specs survive the migration.
const CARD_HEADER = '.panel-heading, .card-header';
const ICON_HIDDEN = /glyphicon-eye-open|bi-eye$/;
const ICON_SHOWN = /glyphicon-eye-close|bi-eye-slash/;

const exactly = (text) => new RegExp(`^${text.replace(/[.*+?^${}()|[\]\\-]/g, '\\$&')}$`);

// '/' bounces through the splash and the password-less user login before the
// master password form appears.
async function gotoLogin(page) {
  await page.goto('/');
  await page.waitForURL(/\/db_login/, { timeout: LOGIN_TIMEOUT });
  await expect(page.getByRole('heading', { name: 'KeePass Login' })).toBeVisible();
}

async function openDb(page, password = MASTER_PASSWORD) {
  await gotoLogin(page);
  await page.getByPlaceholder('Master Password').fill(password);
  await page.getByRole('button', { name: 'Open' }).click();
  await page.waitForURL(/\/keepass/, { timeout: LOGIN_TIMEOUT });
  await expect(treeNode(page, DB.groups[0])).toBeVisible();
}

function treeNode(page, title) {
  return page.locator('.treeview-body .list-group-item').filter({ hasText: exactly(title) });
}

async function selectGroup(page, title) {
  await treeNode(page, title).click();
  await expect(page.locator(`#group-viewer ${CARD_HEADER}`)).toHaveText(title);
}

function entryRows(page) {
  return page.locator('.groupview-body tr');
}

function entryRow(page, title) {
  return entryRows(page).filter({ has: page.locator('td').filter({ hasText: exactly(title) }) });
}

async function openEntry(page, title) {
  await entryRow(page, title).click();
  await expect(page.locator(`#node-viewer ${CARD_HEADER}`)).toHaveText(title);
}

// A row of the entry table, addressed by its label cell ('Password', 'Notes',
// or the name of a custom field).
function fieldRow(page, label) {
  return page.locator('#node-viewer tbody tr')
    .filter({ has: page.locator('td').filter({ hasText: exactly(label) }) });
}

const fieldValue = (page, label) => fieldRow(page, label).locator('td').nth(1);
const revealButton = (page, label) => fieldRow(page, label).locator('button').first();
const copyButton = (page, label) => fieldRow(page, label).locator('button').last();

const clipboard = (page) => page.evaluate(() => navigator.clipboard.readText());

async function expectClipboard(page, value) {
  await expect.poll(() => clipboard(page), { timeout: 10000 }).toBe(value);
}

module.exports = {
  CARD_HEADER,
  DB,
  LOGIN_TIMEOUT,
  MASTER_PASSWORD,
  clipboard,
  copyButton,
  entryRow,
  entryRows,
  exactly,
  expectClipboard,
  fieldRow,
  fieldValue,
  gotoLogin,
  ICON_HIDDEN,
  ICON_SHOWN,
  openDb,
  openEntry,
  revealButton,
  selectGroup,
  treeNode,
};
