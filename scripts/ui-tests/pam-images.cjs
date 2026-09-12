// UI_TEST_URL=http://127.0.0.1:8097 npm run test:pam-images
// Synthetic assets; serve a current Web build. No game installation required.
const assert = require('node:assert/strict');
const os = require('node:os');
const path = require('node:path');
const { deflateSync } = require('node:zlib');
const { chromium } = require('playwright');

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function chunk(kind, payload) {
  const data = Buffer.concat([Buffer.from(kind), payload]);
  const size = Buffer.alloc(4); size.writeUInt32BE(payload.length);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(data));
  return Buffer.concat([size, data, crc]);
}
function png() {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(2, 0); header.writeUInt32BE(1, 4); header[8] = 8; header[9] = 6;
  return Buffer.concat([
    Buffer.from('89504e470d0a1a0a', 'hex'), chunk('IHDR', header),
    chunk('IDAT', deflateSync(Buffer.from([0, 255, 0, 0, 128, 0, 255, 0, 0]))), chunk('IEND', Buffer.alloc(0)),
  ]);
}
function zipEntries(bytes) {
  const entries = [];
  let offset = 0;
  while (offset + 30 <= bytes.length && bytes.readUInt32LE(offset) === 0x04034b50) {
    assert.equal(bytes.readUInt16LE(offset + 8), 0);
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
async function downloadBytes(download) {
  const chunks = [];
  for await (const chunk of await download.createReadStream()) chunks.push(chunk);
  return Buffer.concat(chunks);
}
function fixture(imageNames) {
  return {
    version: 6, frame_rate: 30, position: [0, 0], size: [400, 300], sprite: [],
    image: imageNames.map(name => ({ name, size: [50, 60], transform: [2, 0, 0, 2, 10, 20] })),
    main_sprite: { name: 'main', frame_rate: 30, work_area: [0, 1], frame: [{}] },
  };
}

(async () => {
  const browser = await chromium.launch({ headless: true, ...(process.env.PLAYWRIGHT_CHROMIUM_CHANNEL ? { channel: process.env.PLAYWRIGHT_CHROMIUM_CHANNEL } : {}) });
  const page = await browser.newPage({ viewport: { width: 1440, height: 960 }, acceptDownloads: true });
  page.setDefaultTimeout(30000);
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  try {
    await page.goto(process.env.UI_TEST_URL || 'http://127.0.0.1:8097/');
    await page.getByRole('button', { name: /PAM Editor/ }).first().click({ timeout: 60000 });
    const pam = page.locator('.tk-tool-slot--active .pam-page-host');
    const toolbar = pam.locator('.pam-editor-toolbar');
    const exportButton = toolbar.getByRole('button', { name: /导出所有 Sprites|Export all Sprites/ });
    const saveButton = toolbar.getByRole('button', { name: /保存 PAM|Save PAM/ });
    await toolbar.getByRole('button', { name: /新建动画|New animation/ }).click();
    assert.equal(await exportButton.isDisabled(), true, 'an empty PAM has no images to export');
    const load = async (name, document, images = []) => {
      const input = pam.locator('.pam-new-tab input[type=file]');
      await input.evaluate(element => { element.removeAttribute('webkitdirectory'); element.removeAttribute('directory'); });
      await input.setInputFiles([{ name, mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(document)) }, ...images]);
      await pam.locator('.pam-status-message').filter({ hasText: name }).waitFor();
    };
    await load('assets.pam.json', fixture(['atlas$Leaf[2]|fallback', 'leaf', 'missing']), [
      { name: 'Leaf.png', mimeType: 'image/png', buffer: png() },
    ]);
    assert.equal(await exportButton.isEnabled(), true);
    assert.match(await pam.locator('.pam-status-message').innerText(), /2\/3/);
    const position = await exportButton.boundingBox();
    const savePosition = await saveButton.boundingBox();
    assert.ok(position.x + position.width <= savePosition.x, 'export sits immediately left of Save PAM');
    assert.ok(Math.abs(position.y - savePosition.y) <= 1, 'export and save share a row');
    await pam.locator('.pam-editor-timeline').getByRole('button', { name: /插入保持帧|Insert hold frame/ }).click();
    await toolbar.locator('.pam-save-state.is-dirty').waitFor();
    const dirtyBefore = await toolbar.locator('.pam-save-state').innerText();
    const downloadPromise = page.waitForEvent('download');
    await exportButton.click();
    const download = await downloadPromise;
    assert.equal(download.suggestedFilename(), 'assets_sprites.zip');
    const entries = zipEntries(await downloadBytes(download));
    assert.deepEqual(entries.map(entry => entry.name), ['Leaf.png', 'leaf_2.png', '_missing-images.txt']);
    for (const entry of entries.filter(entry => entry.name.endsWith('.png'))) {
      assert.equal(entry.bytes.readUInt32BE(16), 2, 'original image width, not PAM logical width');
      assert.equal(entry.bytes.readUInt32BE(20), 1, 'original image height, not stage height');
      assert.equal(entry.bytes[25], 6, 'RGBA PNG retains transparency');
    }
    assert.match(entries.at(-1).bytes.toString(), /missing/);
    assert.match(await pam.locator('.pam-status-message').innerText(), /2\/3.*_missing-images.txt/);
    assert.equal(await toolbar.locator('.pam-save-state').innerText(), dirtyBefore);
    assert.equal(await exportButton.isEnabled(), true);
    assert.equal(await saveButton.isEnabled(), true);
    await page.screenshot({ path: path.join(os.tmpdir(), 'pam-export-all-sprites-desktop.png') });
    console.log('PNG ZIP, names, missing report, original dimensions, dirty state and button position passed');
    await page.setViewportSize({ width: 390, height: 844 });
    const bounds = await toolbar.evaluate(element => ({ width: element.clientWidth, scroll: element.scrollWidth }));
    assert.ok(bounds.scroll <= bounds.width, 'narrow toolbar does not overflow');
    assert.equal(await exportButton.isVisible(), true);
    const mobileExport = await exportButton.boundingBox();
    const mobileSave = await saveButton.boundingBox();
    assert.ok(mobileExport.x + mobileExport.width <= mobileSave.x);
    await page.screenshot({ path: path.join(os.tmpdir(), 'pam-export-all-sprites-mobile.png') });
    await load('cancel.pam.json', fixture(Array(256).fill('leaf')), [
      { name: 'leaf.png', mimeType: 'image/png', buffer: png() },
    ]);
    let unexpectedDownload = false;
    const onDownload = () => { unexpectedDownload = true; };
    page.on('download', onDownload);
    await exportButton.click();
    await pam.locator('.pam-export-dialog').getByRole('button', { name: /取消|Cancel/ }).click();
    await pam.locator('.pam-export-dialog').waitFor({ state: 'hidden' });
    page.off('download', onDownload);
    assert.equal(unexpectedDownload, false, 'cancelled exports do not download partial ZIPs');
    console.log('IMAGE ZIP CANCELLATION PASSED');
    await load('missing.pam.json', fixture(['missing']));
    assert.equal(await exportButton.isDisabled(), true, 'all missing assets disable export');
    assert.deepEqual(errors, []);
    console.log('PAM IMAGE EXPORT END-TO-END PASSED');
  } catch (error) {
    console.error('PAGE TEXT', (await page.locator('body').innerText()).slice(-10000));
    await page.screenshot({ path: path.join(os.tmpdir(), 'pam-export-all-sprites-failure.png') });
    throw error;
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exit(1); });
