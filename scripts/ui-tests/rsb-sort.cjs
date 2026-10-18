// Sorting and file-size regressions for the RSB resource explorer.
// Runs against a served Web build:
//   UI_TEST_URL=http://127.0.0.1:8097 npm run test:rsb-sort
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium, webkit } = require('playwright');

const URL = process.env.UI_TEST_URL || 'http://127.0.0.1:8097';
const SAMPLE = process.env.RSB_SORT_SAMPLE || path.join(__dirname, 'fixtures', 'rsb-sort.rsb');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

const parseSize = text => {
  const match = text.trim().match(/^([\d.]+)\s*(B|KB|MB|GB|TB)$/);
  assert.ok(match, `unexpected size text: ${text}`);
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  return Number(match[1]) * 1024 ** units.indexOf(match[2]);
};

async function run(engineName, engine) {
  const browser = await engine.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 960 }, locale: 'en-US' });
  page.setDefaultTimeout(15000);
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));

  await page.goto(URL);
  await page.waitForSelector('.tk-appbar', { timeout: 60000 });
  await sleep(1200);
  await page.locator('.tk-sidebar .tk-nav-item[data-tool="rsb"]').click();
  await sleep(700);

  const tool = page.locator('.tk-tool-slot--active');
  await tool.locator('input[type=file]').first().setInputFiles(SAMPLE);
  await page.waitForFunction(() => {
    const entry = document.querySelector('.tk-tool-slot--active .rsb-resource-entry');
    return entry && !entry.disabled;
  });
  await tool.locator('.rsb-resource-entry').click();
  await tool.locator('.rsb-resource-explorer').waitFor();
  await sleep(800);

  const explorer = tool.locator('.rsb-resource-explorer');
  const headers = await explorer.locator('.rsb-res-table-head button').allInnerTexts();
  assert.match(headers.join(' '), /名称/);
  assert.match(headers.join(' '), /类型 \/ 子组/);
  assert.match(headers.join(' '), /文件大小/);
  assert.match(await explorer.locator('.rsb-res-table-head').innerText(), /映射状态/);

  await explorer.locator('input[aria-label="搜索资源"]').fill('resources');
  await page.waitForFunction(() => {
    const rows = [...document.querySelectorAll('.tk-tool-slot--active .rsb-res-row')];
    return rows.length >= 2 && rows.every(row => {
      const size = row.querySelector('.rsb-res-size')?.textContent?.trim();
      return size && size !== '—';
    });
  });

  const rows = () => explorer.locator('.rsb-res-row').evaluateAll(nodes => nodes.map(node => ({
    name: node.querySelector('.rsb-res-name strong')?.textContent?.trim() || '',
    size: node.querySelector('.rsb-res-size')?.textContent?.trim() || '',
  })));
  const initial = await rows();
  assert.equal(initial.length, 2);
  assert.ok(initial.every(row => row.size !== '—'));

  const sizeHead = explorer.locator('.rsb-res-table-head button').filter({ hasText: '文件大小' });
  await sizeHead.click();
  await sleep(350);
  assert.equal(await sizeHead.getAttribute('aria-sort'), 'ascending');
  const ascending = await rows();
  assert.ok(parseSize(ascending[0].size) <= parseSize(ascending[1].size),
    `size ascending order is wrong: ${JSON.stringify(ascending)}`);

  await sizeHead.click();
  await sleep(350);
  assert.equal(await sizeHead.getAttribute('aria-sort'), 'descending');
  const descending = await rows();
  assert.ok(parseSize(descending[0].size) >= parseSize(descending[1].size),
    `size descending order is wrong: ${JSON.stringify(descending)}`);

  const nameHead = explorer.locator('.rsb-res-table-head button').filter({ hasText: '名称' });
  await nameHead.click();
  await sleep(350);
  assert.equal(await nameHead.getAttribute('aria-sort'), 'ascending');
  const names = (await rows()).map(row => row.name);
  assert.deepEqual(names, [...names].sort((left, right) => left.localeCompare(right)));

  await nameHead.click();
  await sleep(350);
  assert.equal(await nameHead.getAttribute('aria-sort'), 'descending');
  const reversed = (await rows()).map(row => row.name);
  assert.deepEqual(reversed, [...names].reverse());

  assert.deepEqual(errors, [], `${engineName} page errors`);
  await browser.close();
  console.log(`${engineName}: RSB resource sorting and file sizes passed`);
}

(async () => {
  const engines = (process.env.UI_TEST_BROWSERS || 'chromium,webkit').split(',');
  for (const name of engines) {
    if (!['chromium', 'webkit'].includes(name)) throw new Error(`Unsupported UI_TEST_BROWSERS entry: ${name}`);
    await run(name, { chromium, webkit }[name]);
  }
})().catch(error => { console.error(error); process.exit(1); });
