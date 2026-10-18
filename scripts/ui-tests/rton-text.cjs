// Run against a served Web build: UI_TEST_URL=http://127.0.0.1:8097 npm run test:rton-text
const assert = require('node:assert/strict');
const { chromium, webkit } = require('playwright');

(async () => {
  const engine = process.env.RTON_TEST_BROWSER === 'webkit' ? webkit : chromium;
  const browser = await engine.launch({
    headless: true,
    ...(engine === chromium && process.env.PLAYWRIGHT_CHROMIUM_CHANNEL
      ? { channel: process.env.PLAYWRIGHT_CHROMIUM_CHANNEL } : {}),
  });
  const page = await browser.newPage({ viewport: { width: 1440, height: 960 } });
  page.setDefaultTimeout(20000);
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  try {
    await page.goto(process.env.UI_TEST_URL || 'http://127.0.0.1:8097/');
    await page.locator('.tk-sidebar .tk-nav-item[data-tool="rton"]').first().click({ timeout: 60000 });
    const rton = page.locator('.tk-tool-slot--active .rton-page-host');
    const input = rton.locator('#rton-virtual-text-input-sink');
    const read = () => rton.evaluate(root => {
      const lines = [...root.querySelectorAll('.virtual-text-line-view')];
      const caret = root.querySelector('.virtual-text-caret');
      let position = null;
      if (caret) {
        const line = caret.closest('.virtual-text-line-view');
        const range = document.createRange();
        range.selectNodeContents(line);
        range.setEndBefore(caret);
        position = { line: Number(line.dataset.lineNumber), column: range.toString().length };
      }
      return { text: lines.map(line => line.textContent).join('\n'), caret: position };
    });
    const expect = async (text, line, column) => {
      await page.waitForFunction(({ text, line, column }) => {
        const root = document.querySelector('.tk-tool-slot--active .rton-page-host');
        const caret = root?.querySelector('.virtual-text-caret');
        if (!caret) return false;
        const row = caret.closest('.virtual-text-line-view');
        const range = document.createRange();
        range.selectNodeContents(row);
        range.setEndBefore(caret);
        return [...root.querySelectorAll('.virtual-text-line-view')].map(row => row.textContent).join('\n') === text
          && Number(row.dataset.lineNumber) === line && range.toString().length === column;
      }, { text, line, column }, { timeout: 5000 }).catch(async error => {
        console.error('EXPECTED', { text, line, column }, 'ACTUAL', await read());
        throw error;
      });
      assert.deepEqual(await read(), { text, caret: { line, column } });
    };
    let fixtureId = 0;
    const open = async (text, extension = 'yaml') => {
      const name = `cursor-${++fixtureId}.${extension}`;
      await rton.locator('input[type=file]').first().setInputFiles({
        name, mimeType: 'text/plain', buffer: Buffer.from(text),
      });
      await rton.getByText(name, { exact: true }).dblclick();
      const backdrop = rton.locator('.ui-context-sheet-layer--left .ui-context-sheet-backdrop');
      const bounds = await backdrop.boundingBox();
      assert.ok(bounds);
      await backdrop.click({ position: { x: bounds.width - 16, y: 16 } });
      await backdrop.waitFor({ state: 'hidden' });
      await input.waitFor({ state: 'attached' });
      await expect(text, 1, 0);
      await input.focus();
    };
    const point = (lineNumber, column) => rton.locator(`.virtual-text-line-view[data-line-number="${lineNumber}"]`).evaluate((row, column) => {
      const walker = document.createTreeWalker(row, NodeFilter.SHOW_TEXT);
      const nodes = [];
      let node;
      while ((node = walker.nextNode())) if (node.length) nodes.push(node);
      let remaining = column;
      for (const node of nodes) {
        if (remaining < node.length) {
          const range = document.createRange();
          range.setStart(node, remaining);
          range.setEnd(node, remaining + 1);
          const box = range.getClientRects()[0];
          return { x: box.left + 0.25, y: box.top + box.height / 2 };
        }
        remaining -= node.length;
      }
      if (nodes.length) {
        const last = nodes.at(-1);
        const range = document.createRange();
        range.setStart(last, last.length - 1);
        range.setEnd(last, last.length);
        const box = [...range.getClientRects()].at(-1);
        return { x: box.right + 0.25, y: box.top + box.height / 2 };
      }
      const box = row.getBoundingClientRect();
      return { x: box.left + 12, y: box.top + 10 };
    }, column);
    const click = async (line, column, modifiers = []) => {
      const p = await point(line, column);
      for (const modifier of modifiers) await page.keyboard.down(modifier);
      await page.mouse.click(p.x, p.y);
      for (const modifier of modifiers.toReversed()) await page.keyboard.up(modifier);
    };

    await open('abc');
    await page.keyboard.press('End');
    await page.keyboard.type('XYZ');
    await expect('abcXYZ', 1, 6);
    await page.keyboard.type('0123456789'.repeat(20));
    await expect('abcXYZ' + '0123456789'.repeat(20), 1, 206);
    console.log('CONTINUOUS END-OF-LINE TYPING PRESERVES ORDER AND CARET');

    await open('abc');
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    await expect('abc\n', 2, 0);
    await page.keyboard.type('de');
    await expect('abc\nde', 2, 2);
    await page.keyboard.press('Home');
    await page.keyboard.press('Backspace');
    await expect('abcde', 1, 3);
    console.log('TRAILING NEWLINE AND LINE JOIN KEEP THE CORRECT CARET');

    await open('abc');
    await page.keyboard.press('Control+a');
    await page.keyboard.press('Backspace');
    await expect('', 1, 0);
    await page.keyboard.insertText('中文😀');
    await expect('中文😀', 1, 4);
    await page.keyboard.press('ArrowLeft');
    await expect('中文😀', 1, 2);
    await page.keyboard.press('Delete');
    await expect('中文', 1, 2);
    await page.keyboard.insertText('\nnext\n');
    await expect('中文\nnext\n', 3, 0);
    await page.keyboard.type('end');
    await expect('中文\nnext\nend', 3, 3);
    console.log('EMPTY DOCUMENT AND MULTILINE UNICODE INSERTION REMAIN EDITABLE');

    const mixed = 'a\t中文😀bc';
    await open(mixed);
    for (const column of [2, 6, 3, 7, 1, 4, 8, 0]) {
      await click(1, column);
      await expect(mixed, 1, column);
    }
    await page.keyboard.press('End');
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.insertText('X');
    await expect('a\t中文😀bXc', 1, 8);
    console.log('POINTER HIT TESTING HANDLES TABS, CJK, EMOJI AND SPLIT TEXT SPANS');

    await open('abcdef');
    const start = await point(1, 1), end = await point(1, 4);
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(end.x, end.y, { steps: 5 });
    await page.mouse.up();
    await rton.locator('.virtual-text-selection-fragment').filter({ hasText: 'bcd' }).waitFor();
    await page.keyboard.type('Q');
    await expect('aQef', 1, 2);
    await click(1, 1);
    await click(1, 3, ['Shift']);
    await page.keyboard.type('Z');
    await expect('aZf', 1, 2);
    console.log('DRAG AND SHIFT-CLICK SELECTIONS REPLACE AT THE EXPECTED POSITION');

    const expand = rton.getByRole('button', { name: /展开工具栏|Expand toolbar/ });
    if (await expand.count()) await expand.click();
    const wrap = rton.locator('label.rton-switch').filter({ hasText: /^(换行|Wrap)$/ });
    if (!await wrap.locator('input').isChecked()) await wrap.click();
    assert.ok(await wrap.locator('input').isChecked());
    await rton.locator('.ui-tool-drawer-close').click();
    const long = '0123456789'.repeat(36);
    await open(long);
    await rton.locator('.virtual-text-preview.wrap').waitFor();
    await click(1, 250);
    await expect(long, 1, 250);
    await page.keyboard.type('XYZ');
    await expect(long.slice(0, 250) + 'XYZ' + long.slice(250), 1, 253);
    assert.ok(await rton.locator('.virtual-text-preview.wrap').count());
    await page.keyboard.press('Control+z');
    await page.keyboard.press('Control+z');
    await page.keyboard.press('Control+z');
    assert.equal((await read()).text, long);
    await page.keyboard.press('Control+y');
    await page.keyboard.press('Control+y');
    await page.keyboard.press('Control+y');
    assert.equal((await read()).text, long.slice(0, 250) + 'XYZ' + long.slice(250));
    console.log('WRAPPED-LINE CLICKING, EDITING AND UNDO/REDO PASSED');

    await rton.locator('.rton-file-tab.active .rton-file-tab-close').click();
    const unsaved = rton.locator('.rton-unsaved-dialog');
    await unsaved.waitFor();
    assert.equal(
      await page.evaluate(() => !!document.activeElement?.closest('.rton-unsaved-dialog')),
      true,
      'unsaved dialog should take focus',
    );
    for (let index = 0; index < 6; index += 1) {
      await page.keyboard.press('Tab');
      assert.equal(
        await page.evaluate(() => !!document.activeElement?.closest('.rton-unsaved-dialog')),
        true,
        'unsaved dialog should keep focus inside',
      );
    }
    await page.keyboard.press('Escape');
    await unsaved.waitFor({ state: 'detached' });
    assert.equal(
      await page.evaluate(() => document.activeElement?.classList.contains('rton-file-tab-close')),
      true,
      'canceling the unsaved dialog should restore focus to the tab close button',
    );
    console.log('UNSAVED DIALOG FOCUS, TAB CONTAINMENT AND ESCAPE PASSED');
    assert.deepEqual(errors, []);
  } catch (error) {
    console.error('PAGE TEXT', (await page.locator('body').innerText()).slice(-5000));
    throw error;
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
