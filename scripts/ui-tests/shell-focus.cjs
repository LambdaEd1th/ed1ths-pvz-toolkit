// Keyboard and focus regressions for the Toolkit shell and its overlays.
// Run against a served Web build:
//   UI_TEST_URL=http://127.0.0.1:8097 npm run test:shell
const assert = require('node:assert/strict');
const { chromium, webkit } = require('playwright');

const URL = process.env.UI_TEST_URL || 'http://127.0.0.1:8097';
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

const focusInfo = page => page.evaluate(() => {
  const active = document.activeElement;
  return {
    tag: active?.tagName || null,
    cls: String(active?.className || ''),
    text: (active?.textContent || '').trim().slice(0, 30),
    inSettings: !!active?.closest('.tk-settings-panel'),
    inSidebar: !!active?.closest('.tk-sidebar'),
    inDrawer: !!active?.closest('.ui-tool-drawer-panel'),
    inSheet: !!active?.closest('.ui-context-sheet'),
  };
});

const assertFocusIn = async (page, selector, label) => {
  const inside = await page.evaluate(sel => {
    const root = document.querySelector(sel);
    return !!root && root.contains(document.activeElement);
  }, selector);
  assert.ok(inside, `${label}: focus escaped ${selector}: ${JSON.stringify(await focusInfo(page))}`);
};

const assertFocusOutside = async (page, selector, label) => {
  const inside = await page.evaluate(sel => {
    const root = document.querySelector(sel);
    return !!root && root.contains(document.activeElement);
  }, selector);
  assert.ok(!inside, `${label}: focus unexpectedly inside ${selector}`);
};

async function openSettings(page) {
  await page.locator('.tk-settings-button').click();
  await page.waitForSelector('.tk-settings-panel');
  await sleep(250);
}

async function closeSettings(page) {
  if (await page.locator('.tk-settings-panel').count()) {
    await page.locator('.tk-settings-close').click();
    await page.locator('.tk-settings-panel').waitFor({ state: 'detached' });
    await sleep(250);
  }
}

async function ensureSidebar(page, open) {
  const isOpen = await page.evaluate(() => document.querySelector('.tk-sidebar').classList.contains('tk-sidebar--open'));
  if (isOpen !== open) {
    await page.locator('.tk-appbar .tk-icon-button').first().click();
    await sleep(450);
  }
}

async function openToolDrawer(page, slug) {
  await ensureSidebar(page, true);
  await page.locator(`.tk-sidebar .tk-nav-item[data-tool="${slug}"]`).click();
  await sleep(700);
  const handle = page.locator('.tk-tool-slot--active .ui-tool-drawer-handle');
  if (await handle.isVisible()) {
    await handle.click();
    await page.locator('.tk-tool-slot--active .ui-tool-page-toolbar.is-open').waitFor();
    await sleep(450);
  }
  return handle;
}

async function openRtonDrawer(page) {
  return openToolDrawer(page, 'rton');
}

async function run(engineName, engine) {
  const browser = await engine.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 960 },
    locale: 'en-US',
  });
  const page = await context.newPage();
  const step = label => console.log(`${engineName}: ${label}`);
  page.setDefaultTimeout(15000);
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));

  await page.goto(URL);
  await page.waitForSelector('.tk-appbar', { timeout: 60000 });
  await sleep(1200);
  step('shell ready');

  // --- Settings: initial focus, containment, Escape, focus restoration ---
  await openSettings(page);
  step('settings opened');
  await assertFocusIn(page, '.tk-settings-panel', 'settings initial focus');
  assert.equal(
    await page.evaluate(() => document.activeElement?.classList.contains('tk-settings-close')),
    true,
    'settings should focus the close button',
  );
  assert.equal(await page.evaluate(() => document.querySelector('.tk-appbar').inert), true, 'settings should inert the app bar');
  assert.equal(await page.evaluate(() => document.querySelector('.tk-sidebar').inert), true, 'settings should inert the sidebar');
  assert.equal(await page.evaluate(() => document.querySelector('.tk-content-viewport').inert), true, 'settings should inert the content');

  for (let index = 0; index < 24; index += 1) {
    await page.keyboard.press('Tab');
    await assertFocusIn(page, '.tk-settings-panel', `settings tab ${index + 1}`);
  }
  step('settings tab trap passed');
  await page.keyboard.press('Escape');
  await page.locator('.tk-settings-panel').waitFor({ state: 'detached' });
  await sleep(300);
  assert.equal(
    await page.evaluate(() => document.activeElement?.classList.contains('tk-settings-button')),
    true,
    'closing settings should restore focus to the settings button',
  );
  step('settings escape and focus restore passed');

  // --- Language menu: hidden state, Escape, outside click ---
  await openSettings(page);
  assert.equal(await page.evaluate(() => document.querySelector('#tk-settings-language-menu').inert), true, 'closed language menu should be inert');
  assert.equal(
    await page.evaluate(() => {
      const button = document.querySelector('#tk-settings-language-menu button');
      button.focus();
      return document.activeElement === button;
    }),
    false,
    'closed language menu buttons must not be focusable',
  );
  await page.locator('.tk-settings-language-control').click();
  await sleep(250);
  assert.equal(await page.locator('.tk-settings-language.is-open').count(), 1, 'language menu should open');
  assert.equal(await page.evaluate(() => document.querySelector('#tk-settings-language-menu').inert), false, 'open language menu should not be inert');
  await page.keyboard.press('Escape');
  await sleep(250);
  assert.equal(await page.locator('.tk-settings-language.is-open').count(), 0, 'Escape should close the language menu first');
  assert.equal(await page.locator('.tk-settings-panel').count(), 1, 'Escape should keep settings open while closing the menu');
  assert.equal(
    await page.evaluate(() => document.activeElement?.classList.contains('tk-settings-language-control')),
    true,
    'closing the language menu should restore focus to its control',
  );

  await page.locator('.tk-settings-language-control').click();
  await sleep(200);
  await page.locator('.tk-settings-nav button').first().click();
  await sleep(250);
  assert.equal(await page.locator('.tk-settings-language.is-open').count(), 0, 'clicking elsewhere should close the language menu');
  assert.equal(await page.locator('.tk-settings-panel').count(), 1, 'outside click should not close settings');

  await page.locator('.tk-settings-language-control').click();
  await sleep(200);
  await page.keyboard.press('Escape');
  await sleep(200);
  await page.keyboard.press('Escape');
  await page.locator('.tk-settings-panel').waitFor({ state: 'detached' });
  await sleep(300);
  assert.equal(
    await page.evaluate(() => document.activeElement?.classList.contains('tk-settings-button')),
    true,
    'second Escape should close settings and restore focus',
  );

  step('language menu passed');

  // --- Closed sidebar: inert and out of the tab order ---
  await ensureSidebar(page, false);
  assert.equal(await page.evaluate(() => document.querySelector('#tk-sidebar').inert), true, 'closed sidebar should be inert');
  assert.equal(await page.evaluate(() => document.querySelector('#tk-sidebar').getAttribute('aria-hidden')), 'true', 'closed sidebar should be hidden from AT');
  assert.equal(
    await page.evaluate(() => {
      const item = document.querySelector('#tk-sidebar .tk-nav-item');
      item.focus();
      return document.activeElement === item;
    }),
    false,
    'closed sidebar items must not be focusable',
  );
  await page.locator('.tk-appbar .tk-icon-button').first().focus();
  for (let index = 0; index < 8; index += 1) {
    await page.keyboard.press('Tab');
    await assertFocusOutside(page, '#tk-sidebar', `closed sidebar tab ${index + 1}`);
  }
  assert.equal(
    await page.locator('.tk-appbar .tk-icon-button').first().getAttribute('aria-expanded'),
    'false',
    'sidebar toggle should expose aria-expanded=false',
  );
  await ensureSidebar(page, true);
  assert.equal(
    await page.locator('.tk-appbar .tk-icon-button').first().getAttribute('aria-expanded'),
    'true',
    'sidebar toggle should expose aria-expanded=true',
  );

  step('closed sidebar passed');

  // --- Context sheet: focus containment, Escape, focus restoration ---
  const handle = await openRtonDrawer(page);
  const fileList = page.locator('.tk-tool-slot--active button[aria-label="File List"]');
  await fileList.click();
  await page.locator('.ui-context-sheet').waitFor();
  await sleep(300);
  await assertFocusIn(page, '.ui-context-sheet', 'context sheet initial focus');
  assert.equal(await page.locator('.ui-context-sheet').getAttribute('aria-modal'), 'true');
  for (let index = 0; index < 20; index += 1) {
    await page.keyboard.press('Tab');
    await assertFocusIn(page, '.ui-context-sheet', `context sheet tab ${index + 1}`);
  }
  await page.keyboard.press('Escape');
  await page.locator('.ui-context-sheet').waitFor({ state: 'detached' });
  await sleep(300);
  assert.equal(
    await page.evaluate(() => document.activeElement?.getAttribute('aria-label') === 'File List'),
    true,
    'closing a context sheet should restore focus to its trigger',
  );
  await page.keyboard.press('Escape');
  await sleep(250);
  assert.equal(
    await page.locator('.tk-tool-slot--active .ui-tool-page-toolbar.is-open').count(),
    0,
    'Escape should close the docked toolbar',
  );
  assert.equal(
    await page.evaluate(() => document.activeElement?.classList.contains('ui-tool-drawer-handle')),
    true,
    'closing the docked toolbar should restore focus to its handle',
  );

  step('context sheet passed');

  // --- Toolbar labels follow the shell locale ---
  assert.doesNotMatch(await handle.getAttribute('aria-label'), /工具栏|工具/, 'English shell must not show Chinese toolbar chrome');
  await openSettings(page);
  await page.locator('.tk-settings-language-control').click();
  await sleep(200);
  await page.locator('#tk-settings-language-menu button').filter({ hasText: 'Français' }).click();
  await sleep(450);
  assert.equal(await page.evaluate(() => document.documentElement.lang), 'fr-FR', 'language menu should switch the locale');
  assert.match(await handle.getAttribute('aria-label'), /barre d’outils/, 'toolbar chrome should follow the locale');
  assert.equal(
    await page.locator('.tk-sidebar .tk-nav-item').filter({ hasText: 'Archive RSB' }).count(),
    1,
    'sidebar tool names should follow the locale',
  );
  assert.equal(
    (await page.locator('.tk-breadcrumb strong').innerText()).trim(),
    'Éditeur RTON',
    'breadcrumb tool name should follow the locale',
  );
  assert.ok(
    (await page.locator('.tk-workspace-card h3').evaluateAll(nodes => nodes.map(node => node.textContent))).includes('Archive RSB'),
    'home tool cards should follow the locale',
  );
  assert.ok(
    (await page.locator('.tk-about-scroll h3').evaluateAll(nodes => nodes.map(node => node.textContent))).includes('Archive RSB'),
    'about tool headings should follow the locale',
  );
  await page.locator('.tk-settings-language-control').click();
  await sleep(200);
  await page.locator('#tk-settings-language-menu button').filter({ hasText: 'English' }).click();
  await sleep(350);
  assert.equal(await page.evaluate(() => document.documentElement.lang), 'en-US', 'language menu should switch back');
  assert.equal(
    await page.locator('.tk-sidebar .tk-nav-item').filter({ hasText: 'RSB Archive' }).count(),
    1,
    'sidebar tool names should switch back to English',
  );
  await closeSettings(page);

  step('toolbar labels passed');

  // --- Per-tool dialogs use the shared modal focus layer ---
  await openToolDrawer(page, 'newton');
  await page.locator('.tk-tool-slot--active button[aria-label="新建清单"]').click();
  await sleep(400);
  await page.locator('.tk-tool-slot--active .newton-tab-close').first().click();
  await page.locator('.newton-dialog').waitFor();
  await assertFocusIn(page, '.newton-dialog', 'newton dialog initial focus');
  assert.equal(await page.evaluate(() => document.querySelector('.tk-appbar').inert), true, 'newton dialog should make the shell inert');
  for (let index = 0; index < 5; index += 1) {
    await page.keyboard.press('Tab');
    await assertFocusIn(page, '.newton-dialog', `newton dialog tab ${index + 1}`);
  }
  await page.keyboard.press('Escape');
  await page.locator('.newton-dialog').waitFor({ state: 'detached' });
  await sleep(300);
  assert.equal(
    await page.evaluate(() => document.activeElement?.classList.contains('newton-tab-close')),
    true,
    'closing the newton dialog should restore focus to the tab close button',
  );

  await openToolDrawer(page, 'rsb-patch');
  const patchCreate = page.locator('.tk-tool-slot--active button[aria-label="创建补丁"]');
  await patchCreate.click();
  await page.locator('.rsp-dialog').waitFor();
  await assertFocusIn(page, '.rsp-dialog', 'rsb patch dialog initial focus');
  for (let index = 0; index < 5; index += 1) {
    await page.keyboard.press('Tab');
    await assertFocusIn(page, '.rsp-dialog', `rsb patch dialog tab ${index + 1}`);
  }
  await page.keyboard.press('Escape');
  await page.locator('.rsp-dialog').waitFor({ state: 'detached' });
  await sleep(300);
  assert.equal(
    await page.evaluate(() => document.activeElement?.getAttribute('aria-label')),
    '创建补丁',
    'closing the rsb patch dialog should restore focus to its trigger',
  );
  step('tool dialogs passed');

  // --- Mobile: sidebar as a modal, drawer focus handling ---
  const mobile = await context.newPage();
  await mobile.setViewportSize({ width: 390, height: 844 });
  mobile.setDefaultTimeout(15000);
  mobile.on('pageerror', error => errors.push(error.message));
  await mobile.goto(URL);
  await mobile.waitForSelector('.tk-appbar', { timeout: 60000 });
  await sleep(1400);

  assert.equal(await mobile.evaluate(() => document.querySelector('#tk-sidebar').inert), true, 'mobile closed sidebar should be inert');
  assert.equal(
    await mobile.evaluate(() => {
      const item = document.querySelector('#tk-sidebar .tk-nav-item');
      item.focus();
      return document.activeElement === item;
    }),
    false,
    'mobile closed sidebar items must not be focusable',
  );
  assert.equal(await mobile.evaluate(() => document.querySelector('.tk-sidebar-backdrop').inert), true, 'mobile closed sidebar backdrop should be inert');

  await mobile.locator('.tk-appbar .tk-icon-button').first().click();
  await sleep(500);
  await assertFocusIn(mobile, '#tk-sidebar', 'mobile sidebar initial focus');
  for (let index = 0; index < 20; index += 1) {
    await mobile.keyboard.press('Tab');
    await assertFocusIn(mobile, '#tk-sidebar', `mobile sidebar tab ${index + 1}`);
  }
  await mobile.keyboard.press('Escape');
  await sleep(400);
  assert.equal(
    await mobile.evaluate(() => document.querySelector('#tk-sidebar').classList.contains('tk-sidebar--open')),
    false,
    'Escape should close the mobile sidebar',
  );
  assert.equal(
    await mobile.evaluate(() => document.activeElement?.classList.contains('tk-icon-button')),
    true,
    'closing the mobile sidebar should restore focus to the toggle',
  );

  await mobile.locator('.tk-appbar .tk-icon-button').first().click();
  await sleep(450);
  await mobile.locator('.tk-sidebar-backdrop').click({ position: { x: 380, y: 400 } });
  await sleep(450);
  assert.equal(
    await mobile.evaluate(() => document.querySelector('#tk-sidebar').classList.contains('tk-sidebar--open')),
    false,
    'backdrop should close the mobile sidebar',
  );
  assert.equal(
    await mobile.evaluate(() => document.activeElement?.classList.contains('tk-icon-button')),
    true,
    'backdrop close should restore focus to the toggle',
  );

  await mobile.locator('.tk-appbar .tk-icon-button').first().click();
  await sleep(450);
  await mobile.locator('.tk-appbar .tk-icon-button').first().click();
  await sleep(450);
  assert.equal(
    await mobile.evaluate(() => document.querySelector('#tk-sidebar').classList.contains('tk-sidebar--open')),
    false,
    'the topbar toggle should still close the mobile sidebar',
  );

  const mobileHandle = await openRtonDrawer(mobile);
  await assertFocusIn(mobile, '.tk-tool-slot--active .ui-tool-drawer-panel', 'mobile drawer initial focus');
  assert.equal(
    await mobile.locator('.tk-tool-slot--active .ui-tool-drawer-handle').evaluate(element => getComputedStyle(element).visibility),
    'hidden',
    'open mobile drawer should hide its handle from focus',
  );
  for (let index = 0; index < 16; index += 1) {
    await mobile.keyboard.press('Tab');
    await assertFocusIn(mobile, '.tk-tool-slot--active .ui-tool-drawer-panel', `mobile drawer tab ${index + 1}`);
  }
  await mobile.keyboard.press('Escape');
  await sleep(400);
  assert.equal(
    await mobile.evaluate(() => document.querySelector('.tk-tool-slot--active .ui-tool-page-toolbar.is-open')),
    null,
    'Escape should close the mobile drawer',
  );
  assert.equal(
    await mobile.evaluate(() => document.activeElement?.classList.contains('ui-tool-drawer-handle')),
    true,
    'closing the mobile drawer should restore focus to the handle',
  );

  step('mobile drawer passed');
  assert.deepEqual(errors, [], `${engineName} page errors`);
  await browser.close();
  console.log(`${engineName}: shell focus and overlay regressions passed`);
}

(async () => {
  const engines = (process.env.UI_TEST_BROWSERS || 'chromium,webkit').split(',');
  for (const name of engines) {
    if (!['chromium', 'webkit'].includes(name)) throw new Error(`Unsupported UI_TEST_BROWSERS entry: ${name}`);
    await run(name, { chromium, webkit }[name]);
  }
})().catch(error => { console.error(error); process.exit(1); });
