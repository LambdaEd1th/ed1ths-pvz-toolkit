// Optional regression against a locally served build and real game data.
// RSB_RESOURCE_REAL_SAMPLE=/path/to/main.rsb npm run test:rsb-tabs
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { chromium, webkit } = require('playwright');

const sample = process.env.RSB_RESOURCE_REAL_SAMPLE;
if (!sample || !fs.existsSync(sample)) {
  console.log('SKIP: set RSB_RESOURCE_REAL_SAMPLE to an RSB archive');
  process.exit(0);
}

async function dropArchive(page, file, name) {
  // Use a browser File rather than serializing a potentially huge RSB into JS.
  await page.evaluate(() => {
    const input = document.createElement('input');
    input.type = 'file';
    input.id = 'rsb-drag-test-input';
    input.hidden = true;
    document.body.append(input);
  });
  const input = page.locator('#rsb-drag-test-input');
  await input.setInputFiles(file);
  await input.evaluate((element, name) => {
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(new File([element.files[0]], name));
    const target = document.querySelector('.rsb-resource-explorer:not([hidden]) .rsb-res-list')
      || document.querySelector('.rsb-page-host');
    for (const type of ['dragenter', 'dragover', 'drop']) {
      target.dispatchEvent(new DragEvent(type, { dataTransfer, bubbles: true, cancelable: true }));
    }
    element.remove();
  }, name);
}

async function waitForExplorer(page) {
  await page.locator('.rsb-resource-explorer:not([hidden])').waitFor();
  await page.waitForFunction(() => {
    const explorer = document.querySelector('.rsb-resource-explorer:not([hidden])');
    return explorer && [...explorer.querySelectorAll('button')]
      .some(button => button.textContent === '重新扫描' && !button.disabled);
  }, null, { timeout: 60000 });
  console.log('EXPLORER', await page.locator('.rsb-res-statusline').innerText());
}

async function downloadBytes(download) {
  const chunks = [];
  for await (const chunk of await download.createReadStream()) chunks.push(chunk);
  return Buffer.concat(chunks);
}

function packetBytes(file, name) {
  const fd = fs.openSync(file, 'r');
  const header = Buffer.alloc(112);
  try {
    fs.readSync(fd, header, 0, header.length, 0);
    const count = header.readUInt32LE(40);
    const offset = header.readUInt32LE(44);
    const stride = header.readUInt32LE(48);
    const record = Buffer.alloc(stride);
    for (let index = 0; index < count; index++) {
      fs.readSync(fd, record, 0, stride, offset + index * stride);
      if (record.subarray(0, 128).toString().replace(/\0.*$/, '') !== name) continue;
      const bytes = Buffer.alloc(record.readUInt32LE(132));
      fs.readSync(fd, bytes, 0, bytes.length, record.readUInt32LE(128));
      return bytes;
    }
    throw new Error(`packet not found: ${name}`);
  } finally { fs.closeSync(fd); }
}

(async () => {
  const watchdog = setTimeout(() => {
    console.error('FAIL: RSB tab-switch test timed out (possible UI freeze)');
    process.exit(1);
  }, 180000).unref();
  const engine = process.env.RSB_TEST_BROWSER === 'webkit' ? webkit : chromium;
  const browser = await engine.launch({ headless: true, ...(engine === chromium && process.env.PLAYWRIGHT_CHROMIUM_CHANNEL ? { channel: process.env.PLAYWRIGHT_CHROMIUM_CHANNEL } : {}) });
  const page = await browser.newPage({ viewport: { width: 1440, height: 960 } });
  page.setDefaultTimeout(15000);
  const errors = [];
  page.on('pageerror', error => { errors.push(error.message); console.error('PAGEERROR', error.message); });
  page.on('console', message => { if (message.type() === 'error') console.error('CONSOLE', message.text()); });
  let fixtureDirectory;
  try {
    let first = sample;
    let second = process.env.RSB_SECOND_SAMPLE || sample;
    if (process.env.RSB_LARGE_TEST === '1') {
      fixtureDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'rsb-large-tabs-'));
      const sources = [first, second];
      [first, second] = sources.map((source, index) => {
        const file = path.join(fixtureDirectory, `${index}.rsb`);
        fs.copyFileSync(source, file, fs.constants.COPYFILE_FICLONE);
        // Relocate a non-manifest packet into the sparse tail. Manifest reads
        // and the first row's raw export must still be bounded small reads.
        const fd = fs.openSync(file, 'r+');
        try {
          const header = Buffer.alloc(112); fs.readSync(fd, header, 0, header.length, 0);
          for (let item = header.readUInt32LE(40) - 1; item > 0; item--) {
            const position = header.readUInt32LE(44) + item * header.readUInt32LE(48);
            const record = Buffer.alloc(136); fs.readSync(fd, record, 0, record.length, position);
            const name = record.subarray(0, 128).toString().replace(/\0.*$/, '');
            const length = record.readUInt32LE(132);
            if (name.toUpperCase().includes('MANIFEST') || length > 128 * 1024 * 1024) continue;
            const payload = Buffer.alloc(length); fs.readSync(fd, payload, 0, length, record.readUInt32LE(128));
            const offset = Math.ceil(fs.statSync(source).size / 4096) * 4096;
            const size = Math.ceil(Math.max(offset + length, [1_270_000_000, 1_000_000_000][index]) / 4096) * 4096;
            fs.ftruncateSync(fd, size); fs.writeSync(fd, payload, 0, length, offset);
            record.writeUInt32LE(offset, 128); record.writeUInt32LE(size - offset, 132);
            fs.writeSync(fd, record, 0, record.length, position);
            console.log('LARGE PASSTHROUGH PACKET', name, size - offset);
            break;
          }
        } finally { fs.closeSync(fd); }
        return file;
      });
      console.log('LARGE ARCHIVES', fs.statSync(first).size, fs.statSync(second).size);
    }
    await page.addInitScript(() => {
      window.rsbReadStats = { calls: 0, bytes: 0, max: 0, rejected: 0 };
      const record = blob => {
        const stats = window.rsbReadStats;
        stats.calls++;
        stats.bytes += blob.size;
        stats.max = Math.max(stats.max, blob.size);
        if (blob.size > 256 * 1024 * 1024) {
          stats.rejected++;
          throw new Error('Regression guard: attempted to load an entire large RSB into memory');
        }
      };
      const arrayBuffer = Blob.prototype.arrayBuffer;
      Blob.prototype.arrayBuffer = function () { record(this); return arrayBuffer.call(this); };
      const read = FileReader.prototype.readAsArrayBuffer;
      FileReader.prototype.readAsArrayBuffer = function (blob) { record(blob); return read.call(this, blob); };
    });
    await page.goto(process.env.UI_TEST_URL || 'http://127.0.0.1:8097/');
    await page.getByRole('button', { name: /RSB Archive/ }).first().click({ timeout: 60000 });
    await dropArchive(page, first, 'first.rsb');
    await page.getByRole('tab', { name: 'first.rsb', exact: true }).waitFor();
    await page.getByRole('button', { name: '资源管理器', exact: true }).click();
    await waitForExplorer(page);
    await page.locator('[data-resource-folder]').first().dblclick();
    console.log('FIRST FOLDER', await page.getByRole('navigation', { name: '资源路径' }).innerText());
    console.log('DROP SECOND ARCHIVE');
    await dropArchive(page, second, 'second.rsb');
    await page.getByRole('tab', { name: 'second.rsb', exact: true }).waitFor();
    await waitForExplorer(page);
    assert.equal(await page.locator('.rsb-tab-select').count(), 2);
    for (const name of ['first.rsb', 'second.rsb']) {
      await page.getByRole('tab', { name, exact: true }).click();
      await waitForExplorer(page);
      const search = page.getByRole('searchbox', { name: '搜索资源' });
      await search.fill('rsb-tab-regression-no-matching-resource');
      await page.getByText('当前目录没有符合条件的资源', { exact: true }).waitFor();
      await search.fill('');
    }
    await page.getByRole('button', { name: '返回包内文件', exact: true }).click();
    await page.locator('.rsb-page-host').getByRole('button', { name: '展开工具栏', exact: true }).click();
    // Save as must use the browser File directly, without a full read into WASM.
    const readCount = await page.evaluate(() => window.rsbReadStats.calls);
    const [original] = await Promise.all([
      page.waitForEvent('download', { timeout: 30000 }),
      page.getByRole('button', { name: '另存为', exact: true }).click(),
    ]);
    assert.equal(original.suggestedFilename(), 'second.rsb');
    await original.cancel(); // Do not write a multi-GB download just to verify the path.
    assert.equal(await page.evaluate(() => window.rsbReadStats.calls), readCount);

    const row = page.locator('.rsb-table-body .rsb-table-row').first();
    const packetName = await row.locator('strong').innerText();
    await row.click();
    const [rawDownload] = await Promise.all([
      page.waitForEvent('download', { timeout: 30000 }),
      page.getByRole('button', { name: '提取所选项目', exact: true }).click(),
    ]);
    const raw = await downloadBytes(rawDownload);
    assert.deepEqual(raw, packetBytes(second, packetName));
    await page.locator('input[type=file][accept*=".rsg"]').first().setInputFiles({ name: 'replacement.rsg', mimeType: 'application/octet-stream', buffer: raw });
    await page.waitForFunction(() => document.querySelector('.rsb-status-message')?.textContent.startsWith('已替换'));
    await page.getByRole('button', { name: '重命名所选项目', exact: true }).click();
    const rename = page.getByRole('dialog', { name: '重命名', exact: true });
    await rename.getByLabel('新名称').fill('RSB_TAB_READ_TEST');
    await rename.getByRole('button', { name: '应用', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('.rsb-status-message')?.textContent.startsWith('已将 RSG 重命名为 RSB_TAB_READ_TEST'));
    const [edited] = await Promise.all([
      page.waitForEvent('download', { timeout: 60000 }),
      page.getByRole('button', { name: '保存', exact: true }).click(),
    ]);
    await edited.cancel();
    await page.waitForFunction(() => document.querySelector('.rsb-status-message')?.textContent.includes('已保存并重新验证'), null, { timeout: 60000 });
    await page.getByRole('searchbox', { name: '搜索当前位置', exact: true }).fill('RSB_TAB_READ_TEST');
    await page.locator('.rsb-table-body .rsb-table-row').filter({ hasText: 'RSB_TAB_READ_TEST' }).waitFor();
    await page.getByRole('tab', { name: 'first.rsb', exact: true }).click();
    assert.equal(await page.getByRole('tab', { name: 'first.rsb', exact: true }).getAttribute('aria-selected'), 'true');
    console.log('LAZY RAW EXPORT / SAVE AS / REPLACE / RENAME / LARGE COMPOSED SAVE PASSED');
    assert.deepEqual(errors, [], 'switching archives must not panic');
    const reads = await page.evaluate(() => window.rsbReadStats);
    console.log('BOUNDED FILE READS', reads);
    assert.equal(reads.rejected, 0, 'no whole-file reads for large archives');
    assert.ok(reads.max < Math.min(fs.statSync(first).size, fs.statSync(second).size), 'indexing must not read the whole RSB');
    console.log('RSB DROP / RESOURCE EXPLORER / TAB SWITCH PASSED');
  } finally {
    await browser.close();
    if (fixtureDirectory) fs.rmSync(fixtureDirectory, { recursive: true, force: true });
    clearTimeout(watchdog);
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
