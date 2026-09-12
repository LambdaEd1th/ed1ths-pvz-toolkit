// Optional end-to-end test against a locally served build and real game data.
// RSB_RESOURCE_REAL_SAMPLE=/path/to/main.rsb UI_TEST_URL=http://127.0.0.1:8097 npm run test:resources
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { chromium } = require('playwright');
const sample = process.env.RSB_RESOURCE_REAL_SAMPLE;
if (!sample || !fs.existsSync(sample)) { console.log('SKIP: set RSB_RESOURCE_REAL_SAMPLE to an international main.rsb'); process.exit(0); }

async function downloadBytes(download) {
  const chunks = [];
  for await (const chunk of await download.createReadStream()) chunks.push(chunk);
  return Buffer.concat(chunks);
}
function zipEntries(bytes) {
  const entries = [];
  let offset = 0;
  while (offset + 30 <= bytes.length && bytes.readUInt32LE(offset) === 0x04034b50) {
    assert.equal(bytes.readUInt16LE(offset + 8), 0, 'resource ZIP uses stored entries');
    const size = bytes.readUInt32LE(offset + 18);
    const nameLength = bytes.readUInt16LE(offset + 26);
    const extraLength = bytes.readUInt16LE(offset + 28);
    const name = bytes.subarray(offset + 30, offset + 30 + nameLength).toString();
    const start = offset + 30 + nameLength + extraLength;
    entries.push({ name, bytes: bytes.subarray(start, start + size) });
    offset = start + size;
  }
  return entries;
}

(async () => {
  const browser = await chromium.launch({ headless: true, ...(process.env.PLAYWRIGHT_CHROMIUM_CHANNEL ? { channel: process.env.PLAYWRIGHT_CHROMIUM_CHANNEL } : {}) });
  const page = await browser.newPage({ viewport: { width: 1440, height: 960 }, colorScheme: 'dark', acceptDownloads: true });
  page.setDefaultTimeout(30000);
  const errors = [];
  page.on('pageerror', error => { errors.push(error.message); console.error('PAGEERROR', error.message); });
  try {
    await page.goto(process.env.UI_TEST_URL || 'http://127.0.0.1:8097/');
    await page.getByRole('button', { name: /RSB Archive/ }).first().click({ timeout: 60000 });
    await page.locator('.rsb-page-host input[type=file][accept*=".rsb"]').first().setInputFiles(sample);
    await page.getByRole('button', { name: '资源管理器', exact: true }).click();
    const explorer = page.locator('.rsb-resource-explorer');
    const search = explorer.getByRole('searchbox', { name: '搜索资源' });
    await page.waitForFunction(() => document.querySelector('.rsb-res-statusline')?.textContent.includes('24125'), null, { timeout: 60000 });
    console.log('READY', await explorer.locator('.rsb-res-statusline').innerText());
    const manifestSources = explorer.locator('.rsb-res-footer details').filter({ hasText: '已合并' });
    assert.match(await manifestSources.locator('summary').innerText(), /已合并 2 份清单/);
    await manifestSources.locator('summary').click();
    assert.match(await manifestSources.innerText(), /RESOURCES\.RTON/i);
    assert.match(await manifestSources.innerText(), /RESOURCES\.NEWTON/i);
    await manifestSources.locator('summary').click();
    const iconView = explorer.getByRole('button', { name: '图标视图', exact: true });
    await iconView.hover();
    await page.getByRole('tooltip').waitFor();
    assert.equal(await page.getByRole('tooltip').innerText(), '图标视图');
    assert.equal(await iconView.getAttribute('title'), '', 'native bubbles must be disabled');
    await iconView.click();
    await page.getByRole('tooltip').waitFor({ state: 'hidden' });
    await explorer.getByRole('button', { name: '列表视图', exact: true }).click();
    console.log('MERGED MANIFEST SOURCES AND LIVE TOOLTIP DISMISSAL PASSED');

    await search.fill('IMAGE_SPACE_SPIRAL_RADIAL_SPACE_GRADIENT');
    const child = explorer.locator('[data-resource-id="IMAGE_SPACE_SPIRAL_RADIAL_SPACE_GRADIENT"]');
    await child.click();
    assert.match(await explorer.locator('.rsb-res-details').innerText(), /RESOURCES\.RTON/i);
    assert.match(await explorer.locator('.rsb-res-details').innerText(), /RESOURCES\.NEWTON/i);
    await child.dblclick();
    const dialog = explorer.getByRole('dialog', { name: '资源图片预览' });
    await dialog.waitFor();
    await page.waitForFunction(() => { const img = document.querySelector('.rsb-res-image-stage img'); return img?.complete && img.naturalWidth === 82; });
    assert.match(await dialog.innerText(), /82 × 82/);
    await dialog.getByRole('combobox', { name: '预览缩放' }).selectOption('4');
    assert.equal(await dialog.locator('img').evaluate(img => img.getBoundingClientRect().width), 328);
    const previewDownload = page.waitForEvent('download');
    await dialog.getByRole('button', { name: '导出 PNG', exact: true }).click();
    const previewPng = await downloadBytes(await previewDownload);
    assert.equal(previewPng.readUInt32BE(16), 82);
    assert.equal(previewPng.readUInt32BE(20), 82);
    await page.screenshot({ path: path.join(os.tmpdir(), 'rsb-resource-child-preview.png') });
    await dialog.getByRole('button', { name: '关闭预览' }).click();
    const singleDownload = page.waitForEvent('download');
    await explorer.getByRole('button', { name: '导出所选 (1)', exact: true }).click();
    const single = await singleDownload;
    assert.match(single.suggestedFilename(), /\.png$/);
    assert.deepEqual(await downloadBytes(single), previewPng);
    console.log('CHILD PREVIEW AND SINGLE EXPORT PASSED');

    // NEWTON omits ax=0. The merged definition must still be a cropped image,
    // with both source names, rather than an uncroppable atlas child.
    await search.fill('IMAGE_MAINMENU_BACKGROUND');
    await explorer.locator('[data-resource-id="IMAGE_MAINMENU_BACKGROUND"]').dblclick();
    await dialog.waitFor();
    await page.waitForFunction(() => { const img = document.querySelector('.rsb-res-image-stage img'); return img?.complete && img.naturalWidth === 1024 && img.naturalHeight === 768; });
    assert.match(await dialog.innerText(), /1024 × 768/);
    const zeroCoordinateDownload = page.waitForEvent('download');
    await dialog.getByRole('button', { name: '导出 PNG', exact: true }).click();
    const zeroCoordinatePng = await downloadBytes(await zeroCoordinateDownload);
    assert.equal(zeroCoordinatePng.readUInt32BE(16), 1024);
    assert.equal(zeroCoordinatePng.readUInt32BE(20), 768);
    await dialog.getByRole('button', { name: '关闭预览' }).click();
    console.log('ZERO-COORDINATE ATLAS CHILD PREVIEW AND EXPORT PASSED');

    await search.fill('IMAGE_SPACE_SPIRAL');
    const rows = explorer.locator('[data-resource-id]');
    await rows.first().click();
    await rows.nth(1).click({ modifiers: [process.platform === 'darwin' ? 'Meta' : 'Control'] });
    await explorer.getByRole('button', { name: '导出所选 (2)', exact: true }).waitFor();
    await rows.nth(2).click({ modifiers: ['Shift'] });
    // Shift range starts at the last explicitly selected item.
    await explorer.getByRole('button', { name: '导出所选 (2)', exact: true }).waitFor();
    await explorer.getByRole('button', { name: '图标视图', exact: true }).click();
    await explorer.getByRole('button', { name: '导出所选 (2)', exact: true }).waitFor();
    await explorer.getByRole('button', { name: '列表视图', exact: true }).click();
    await explorer.getByRole('button', { name: '导出所选 (2)', exact: true }).waitFor();
    await explorer.getByRole('button', { name: '全选', exact: true }).click();
    const batchDownload = page.waitForEvent('download');
    await explorer.getByRole('button', { name: /^导出所选/ }).click();
    const batch = await batchDownload;
    const entries = zipEntries(await downloadBytes(batch));
    assert.ok(entries.length >= 3);
    assert.ok(entries.every(entry => entry.name.startsWith('images/') && entry.name.endsWith('.png')));
    assert.equal(new Set(entries.map(entry => entry.name.toLowerCase())).size, entries.length);
    assert.ok(entries.every(entry => entry.bytes.subarray(1, 4).toString() === 'PNG'));
    console.log('MULTI-SELECTION AND ZIP PASSED', entries.map(entry => entry.name));

    const home = () => explorer.getByRole('navigation', { name: '资源路径' }).getByRole('button', { name: '逻辑资源', exact: true }).click();
    const folder = name => explorer.locator('[data-resource-folder="' + name + '"]');
    await home();
    await folder('images').dblclick();
    await folder('768').dblclick();
    await folder('initial').dblclick();
    const manual = { groups: [{ id: 'AlwaysLoaded_768', parent: 'AlwaysLoaded', res: '768', resources: [{ id: 'TEST_MISSING_EXPORT', type: 'File', path: 'images/768/initial/space_spiral/unmapped' }] }] };
    await explorer.locator('.rsb-res-import input').setInputFiles({ name: 'export-test.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(manual)) });
    await page.waitForFunction(() => document.querySelector('.rsb-res-footer')?.textContent.includes('24553'));
    for (let step = 0; step < 30 && !await folder('space_spiral').count(); step++) {
      await explorer.locator('.rsb-res-list').evaluate(element => { element.scrollTop += element.clientHeight * 0.75; element.dispatchEvent(new Event('scroll')); });
      await page.waitForTimeout(50);
    }
    await folder('space_spiral').click();
    const folderDownload = page.waitForEvent('download');
    await explorer.getByRole('button', { name: '导出所选 (5)', exact: true }).click();
    const folderEntries = zipEntries(await downloadBytes(await folderDownload));
    assert.equal(folderEntries.length, 5);
    assert.equal(folderEntries.filter(entry => entry.name.endsWith('.png')).length, 4);
    assert.match(folderEntries.find(entry => entry.name === '_export-errors.txt').bytes.toString(), /unmapped/);
    console.log('RECURSIVE FOLDER EXPORT AND SKIP REPORT PASSED');
    await home();
    await folder('images').click();
    let unexpectedDownload = false;
    const onUnexpectedDownload = () => { unexpectedDownload = true; };
    page.on('download', onUnexpectedDownload);
    await explorer.getByRole('button', { name: /^导出所选/ }).click();
    await explorer.getByRole('button', { name: '取消任务', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('.rsb-res-footer')?.textContent.includes('已取消导出，未保存文件'));
    page.off('download', onUnexpectedDownload);
    assert.equal(unexpectedDownload, false);
    console.log('BATCH CANCELLATION PASSED');

    await search.fill('POPANIM_PLANT_ELECTRIC_PEASHOOTER');
    await explorer.locator('[data-resource-id="POPANIM_PLANT_ELECTRIC_PEASHOOTER"]').dblclick();
    await page.waitForFunction(() => document.querySelector('.tk-tool-slot--active .pam-page-host')?.textContent.includes('46/46'), null, { timeout: 90000 });
    await page.waitForFunction(() => document.querySelector('#pam-stage-canvas')?.dataset.rendererBackend, null, { timeout: 60000 });
    await page.waitForTimeout(500); // The first GPU frame follows renderer initialization.
    console.log('PAM RENDERER', await page.evaluate(() => document.documentElement.dataset.renderWorkerBackend));
    console.log('PAM OPENED WITH ALL 46 ATLAS CHILDREN');
    const pamImagesDownload = page.waitForEvent('download');
    await page.locator('.tk-tool-slot--active .pam-editor-toolbar').getByRole('button', { name: '导出所有 Sprites', exact: true }).click();
    const pamImagesZip = zipEntries(await downloadBytes(await pamImagesDownload));
    assert.equal(pamImagesZip.length, 46);
    assert.ok(pamImagesZip.every(entry => entry.name.endsWith('.png') && entry.bytes.subarray(1, 4).toString() === 'PNG'));
    assert.equal(new Set(pamImagesZip.map(entry => entry.name.toLowerCase())).size, 46);
    console.log('ALL 46 PAM SOURCE PNGS EXPORTED');
    await page.screenshot({ path: path.join(os.tmpdir(), 'rsb-resource-pam-open.png') });

    await page.getByRole('button', { name: /Toggle navigation|切换导航/ }).click();
    await page.getByRole('button', { name: /RSB Archive/ }).first().click();
    await explorer.getByRole('combobox', { name: '映射状态' }).selectOption('unlisted');
    await search.fill('resources');
    const manifestRow = explorer.locator('[data-resource-id]').filter({ has: page.locator('.rsb-res-name strong', { hasText: /resources.rton/i }) });
    await manifestRow.click();
    const selectedDetails = await explorer.locator('.rsb-res-details').innerText();
    console.log('MANIFEST HANDOFF', selectedDetails);
    await explorer.getByRole('button', { name: '打开', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('.tk-breadcrumb strong')?.textContent === 'RTON Editor', null, { timeout: 60000 });
    await page.waitForFunction(() => document.querySelector('.tk-tool-slot--active')?.textContent.match(/RESOURCES|resources/), null, { timeout: 60000 });
    console.log('RTON HANDOFF PASSED');

    await page.getByRole('button', { name: /Toggle navigation|切换导航/ }).click();
    await page.getByRole('button', { name: /RSB Archive/ }).first().click();
    await explorer.getByRole('combobox', { name: '映射状态' }).selectOption('all');
    await search.fill('.bnk');
    const bankRow = explorer.locator('[data-resource-id]').filter({ has: page.locator('.rsb-res-kind > span', { hasText: /SoundBank/ }) }).first();
    await bankRow.click();
    await explorer.getByRole('button', { name: '打开', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('.tk-breadcrumb strong')?.textContent === 'BNK Archive');
    await page.locator('.tk-tool-slot--active .bnk-tab-select').first().waitFor();
    assert.match(await page.locator('.tk-tool-slot--active .bnk-tab-name').first().innerText(), /\.bnk$/i);
    console.log('BNK IN-MEMORY HANDOFF PASSED');
    await page.getByRole('button', { name: /Toggle navigation|切换导航/ }).click();
    await page.getByRole('button', { name: /RSB Archive/ }).first().click();
    await explorer.getByRole('combobox', { name: '打开方式' }).selectOption('SMF');
    await explorer.getByRole('button', { name: '打开', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('.tk-breadcrumb strong')?.textContent === 'SMF Container');
    await page.locator('.tk-tool-slot--active .smf-tab-select').first().waitFor();
    console.log('EXPLICIT OPEN-WITH HANDOFF PASSED');
    await page.getByRole('button', { name: /Toggle navigation|切换导航/ }).click();
    await page.getByRole('button', { name: /RSB Archive/ }).first().click();
    await explorer.getByRole('navigation', { name: '资源路径' }).getByRole('button', { name: '逻辑资源', exact: true }).click();
    await page.setViewportSize({ width: 390, height: 844 });
    const overflow = await explorer.evaluate(element => ({ width: element.clientWidth, scroll: element.scrollWidth }));
    assert.ok(overflow.scroll <= overflow.width, 'mobile toolbar must not overflow');
    await page.screenshot({ path: path.join(os.tmpdir(), 'rsb-resource-actions-mobile.png') });
    assert.deepEqual(errors, []);
    console.log('RESOURCE ACTIONS END-TO-END PASSED');
  } catch (error) {
    console.error('PAGE TEXT', (await page.locator('body').innerText()).slice(-14000));
    await page.screenshot({ path: path.join(os.tmpdir(), 'rsb-resource-actions-failure.png') });
    throw error;
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exit(1); });
