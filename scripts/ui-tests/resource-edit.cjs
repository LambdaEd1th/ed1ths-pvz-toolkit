// Generate target/resource-edit.rsb with the optional Rust edit_fixture test first.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { chromium, webkit } = require('playwright');
const fixture = process.env.RSB_EDIT_FIXTURE || path.resolve(__dirname, '../../target/resource-edit.rsb');
if (!fs.existsSync(fixture)) throw new Error('Generate target/resource-edit.rsb with RSB_EDIT_FIXTURE_PATH and cargo test -p rsb-tool --lib write_browser_fixture_when_requested');
async function bytes(download) {
  const chunks = []; for await (const chunk of await download.createReadStream()) chunks.push(chunk); return Buffer.concat(chunks);
}

(async () => {
  const engine = process.env.RSB_TEST_BROWSER === 'webkit' ? webkit : chromium;
  const browser = await engine.launch({ headless: true, ...(engine === chromium && process.env.PLAYWRIGHT_CHROMIUM_CHANNEL ? { channel: process.env.PLAYWRIGHT_CHROMIUM_CHANNEL } : {}) });
  const page = await browser.newPage({ viewport: { width: 1440, height: 960 }, colorScheme: 'dark', acceptDownloads: true });
  page.setDefaultTimeout(30000);
  const errors = []; page.on('pageerror', error => { errors.push(error.message); console.error('PAGEERROR', error.message); });
  const original = fs.readFileSync(fixture);
  try {
    await page.goto(process.env.UI_TEST_URL || 'http://127.0.0.1:8097/');
    await page.getByRole('button', { name: /RSB Archive/ }).first().click({ timeout: 60000 });
    await page.locator('.rsb-page-host input[type=file][accept*=".rsb"]').first().setInputFiles(fixture);
    await page.getByRole('button', { name: '资源管理器', exact: true }).click();
    const explorer = page.locator('.rsb-resource-explorer:not([hidden])');
    const search = explorer.getByRole('searchbox', { name: '搜索资源' });
    async function select(id) {
      await search.fill(id);
      const row = explorer.locator('[data-resource-id="' + id + '"]'); await row.click(); return row;
    }
    const editor = page.getByRole('dialog', { name: '编辑资源定义', exact: true });
    await select('LEAF');
    await explorer.getByRole('button', { name: '编辑定义', exact: true }).click();
    await editor.getByRole('spinbutton', { name: '裁剪 X', exact: true }).fill('4');
    await editor.getByRole('button', { name: '应用修改', exact: true }).click();
    await editor.getByRole('alert').filter({ hasText: '超出父图集' }).waitFor();
    await editor.getByRole('button', { name: '取消', exact: true }).click();
    assert.equal(await explorer.locator('[data-resource-id="LEAF"]').count(), 1);
    console.log('OUT-OF-BOUNDS EDIT ROLLED BACK');

    await select('ATLAS');
    await explorer.getByRole('button', { name: '编辑定义', exact: true }).click();
    await editor.getByRole('textbox', { name: '资源 ID', exact: true }).fill('RENAMED_ATLAS');
    await editor.getByRole('button', { name: '应用修改', exact: true }).click();
    await editor.waitFor({ state: 'hidden' });
    await select('LEAF');
    assert.match(await explorer.locator('.rsb-res-details').innerText(), /RENAMED_ATLAS/);
    assert.match(await explorer.locator('.rsb-res-footer [role=status]').innerText(), /同步 2 份清单/);
    console.log('ATLAS RENAME UPDATED BOTH MANIFESTS AND CHILD REFERENCES');

    const png = async size => Buffer.from(await page.evaluate(size => {
      const canvas = document.createElement('canvas'); canvas.width = canvas.height = size;
      return canvas.toDataURL('image/png').split(',')[1];
    }, size), 'base64');
    await explorer.getByRole('button', { name: '替换内容', exact: true }).click();
    const replace = page.getByRole('dialog', { name: '替换资源内容', exact: true });
    await replace.getByLabel('选择资源内容文件').setInputFiles({ name: 'wrong.png', mimeType: 'image/png', buffer: await png(1) });
    await replace.getByRole('button', { name: '应用修改', exact: true }).click();
    await replace.getByRole('alert').filter({ hasText: '必须保持' }).waitFor();
    await replace.getByLabel('选择资源内容文件').setInputFiles({ name: 'transparent.png', mimeType: 'image/png', buffer: await png(2) });
    await replace.getByRole('button', { name: '应用修改', exact: true }).click();
    await replace.waitFor({ state: 'hidden' });
    await (await select('LEAF')).dblclick();
    const preview = explorer.getByRole('dialog', { name: '资源图片预览' });
    await preview.waitFor();
    await page.waitForFunction(() => { const img = document.querySelector('.rsb-res-image-stage img'); return img?.complete && img.naturalWidth === 2; });
    assert.deepEqual(await preview.locator('img').evaluate(img => {
      const canvas = document.createElement('canvas'); canvas.width = canvas.height = 2;
      const ctx = canvas.getContext('2d'); ctx.drawImage(img, 0, 0); return [...ctx.getImageData(0, 0, 2, 2).data];
    }), Array(16).fill(0));
    await preview.getByRole('button', { name: '关闭预览' }).click();
    console.log('SAME-SIZE TRANSPARENT CHILD REPLACEMENT VERIFIED');

    await select('CONFIG');
    await explorer.getByRole('button', { name: '新增定义', exact: true }).click();
    const add = page.getByRole('dialog', { name: '新增资源定义', exact: true });
    await add.getByRole('textbox', { name: '资源 ID', exact: true }).fill('ADDED_FILE');
    await add.getByRole('textbox', { name: '逻辑路径', exact: true }).fill('data/added.txt');
    await add.getByLabel('选择资源内容文件').setInputFiles({ name: 'added.txt', mimeType: 'text/plain', buffer: Buffer.from('added content') });
    await page.screenshot({ path: path.join(os.tmpdir(), 'rsb-resource-editor.png') });
    await page.setViewportSize({ width: 390, height: 844 });
    assert.ok(await add.evaluate(el => el.getBoundingClientRect().width <= window.innerWidth));
    await page.screenshot({ path: path.join(os.tmpdir(), 'rsb-resource-editor-mobile.png') });
    await page.setViewportSize({ width: 1440, height: 960 });
    await add.getByRole('button', { name: '应用修改', exact: true }).click();
    await add.waitFor({ state: 'hidden' });
    await select('ADDED_FILE');
    const [added] = await Promise.all([page.waitForEvent('download'), explorer.getByRole('button', { name: '导出所选 (1)', exact: true }).click()]);
    assert.equal((await bytes(added)).toString(), 'added content');
    await explorer.getByRole('button', { name: '删除定义', exact: true }).click();
    await page.getByRole('dialog', { name: '删除资源定义', exact: true }).getByRole('button', { name: '确认删除定义', exact: true }).click();
    await page.getByRole('dialog', { name: '删除资源定义', exact: true }).waitFor({ state: 'hidden' });
    await search.fill('added.txt');
    await explorer.locator('[data-resource-id]').filter({ hasText: '未列入清单' }).waitFor();
    console.log('ADD CONTENT AND DEFINITION / DELETE DEFINITION WITHOUT DELETING FILE PASSED');

    const [saved] = await Promise.all([page.waitForEvent('download'), explorer.getByRole('button', { name: '保存 RSB', exact: true }).click()]);
    const savedBytes = await bytes(saved);
    await page.waitForFunction(() => document.querySelector('.rsb-status-message')?.textContent.includes('已保存并重新验证'));
    assert.notDeepEqual(savedBytes, original);
    assert.deepEqual(fs.readFileSync(fixture), original);
    await page.locator('.rsb-page-host input[type=file][accept*=".rsb"]').first().setInputFiles({ name: 'reopened.rsb', mimeType: 'application/octet-stream', buffer: savedBytes });
    await select('LEAF');
    assert.match(await explorer.locator('.rsb-res-details').innerText(), /RENAMED_ATLAS/);
    assert.match(await explorer.locator('.rsb-res-details').innerText(), /RESOURCES\.RTON/i);
    assert.match(await explorer.locator('.rsb-res-details').innerText(), /RESOURCES\.NEWTON/i);
    console.log('EDITED DOWNLOAD REOPENED; ORIGINAL FIXTURE UNCHANGED');
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
