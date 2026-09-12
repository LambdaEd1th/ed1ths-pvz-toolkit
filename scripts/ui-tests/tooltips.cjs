// Deterministic lifecycle coverage; no WASM build or game assets required.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium, webkit } = require('playwright');
const root = path.resolve(__dirname, '../..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const script = read('crates/ui/toolkit-ui/assets/tooltips.js');
const css = ['crates/ui/toolkit-ui/assets/tokens.css', 'crates/ui/toolkit-ui/assets/primitives.css'].map(read).join('\n');

(async () => {
  for (const engine of (process.env.UI_TEST_BROWSERS || 'chromium,webkit').split(',')) {
    const browser = await ({ chromium, webkit })[engine].launch({ headless: true,
      ...(engine === 'chromium' && process.env.PLAYWRIGHT_CHROMIUM_CHANNEL ? { channel: process.env.PLAYWRIGHT_CHROMIUM_CHANNEL } : {}) });
    const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    try {
      await page.setContent(`<style>${css}
        body { margin: 0; padding: 30px; } button { padding: 12px; margin: 8px; }
        #scroller { height: 80px; overflow: auto; } #spacer { height: 400px; }
        #overlay { position: fixed; inset: 0; z-index: 100; background: white; }
        #edge { position: fixed; right: 0; bottom: 0; margin: 0; }
      </style>
      <button id="before">Before</button>
      <section id="page"><button id="trigger" title="路径提示\n第二行" aria-describedby="existing">资源</button><button id="other" title="其他提示">其他</button></section>
      <span id="existing">Existing description</span>
      <div id="scroller"><div id="spacer">Scroll here</div></div>
      <section class="ui-tool-page-toolbar"><div class="ui-tool-drawer-body"><div class="ui-tool-page-actions">
        <button id="icon" class="rsb-tool-button--icon" title="归档目录"><svg width="16" height="16" aria-hidden="true"></svg></button>
      </div></div></section>
      <button id="edge" title="This is a long tooltip near the edge of the viewport">Edge</button>`);
      await page.addScriptTag({ content: script });
      const trigger = page.locator('#trigger');
      const tip = page.locator('.ui-tooltip');
      const away = () => page.mouse.move(5, 5);
      const show = async (selector = '#trigger') => {
        await away();
        await page.locator(selector).hover();
        await page.locator('.ui-tooltip:not([hidden])').waitFor({ state: 'visible' });
      };
      const hidden = async () => {
        if (await tip.count()) await tip.waitFor({ state: 'hidden' });
      };
      assert.equal(await trigger.getAttribute('title'), '', 'native title is suppressed');
      assert.equal(await trigger.getAttribute('data-ui-tooltip'), '路径提示\n第二行');
      const caption = await page.locator('#icon').evaluate(element => getComputedStyle(element, '::after').content);
      assert.equal(caption, '"归档目录"', 'persistent toolbar caption remains visible');
      assert.equal(await page.locator('#icon').getAttribute('aria-label'), '归档目录', 'icon-only accessible name is preserved');

      await show();
      assert.equal(await tip.innerText(), '路径提示\n第二行');
      assert.match(await trigger.getAttribute('aria-describedby'), /existing toolkit-shared-tooltip/);
      await away(); await hidden();
      assert.equal(await trigger.getAttribute('aria-describedby'), 'existing');
      await trigger.hover(); await away();
      await page.waitForTimeout(520); await hidden(); // Cancel a pending delayed show.

      await show(); await trigger.click(); await hidden();
      await page.waitForTimeout(520); await hidden(); // Click focus must not resurrect it.
      await show(); await page.keyboard.press('Escape'); await hidden();
      await page.waitForTimeout(520); await hidden();
      await show(); await page.locator('#scroller').evaluate(element => { element.scrollTop = 50; }); await hidden();
      await show(); await page.setViewportSize({ width: 760, height: 580 }); await hidden();
      await show(); await page.evaluate(() => window.dispatchEvent(new Event('blur'))); await hidden();
      await show(); await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange'))); await hidden();
      await show(); await page.evaluate(() => document.dispatchEvent(new PointerEvent('pointercancel', { bubbles: true }))); await hidden();
      await show(); await page.evaluate(() => document.dispatchEvent(new Event('dragstart', { bubbles: true }))); await hidden();
      await show(); await page.evaluate(() => document.querySelector('#page').setAttribute('aria-hidden', 'true')); await hidden();
      await page.evaluate(() => document.querySelector('#page').removeAttribute('aria-hidden'));

      await show();
      await page.evaluate(() => { const overlay = document.createElement('div'); overlay.id = 'overlay'; overlay.setAttribute('role', 'dialog'); document.body.append(overlay); });
      await hidden();
      await page.locator('#overlay').evaluate(element => element.remove());
      await show();
      await trigger.evaluate(element => element.style.transform = 'translateX(100px)');
      await hidden(); // Moving a row without a mouse event invalidates the old hit target.
      await trigger.evaluate(element => element.style.transform = '');

      await show();
      await trigger.evaluate(element => element.setAttribute('title', '新提示'));
      await hidden();
      await show(); assert.equal(await tip.innerText(), '新提示');
      await trigger.evaluate(element => element.setAttribute('title', ''));
      await hidden();
      assert.equal(await trigger.getAttribute('data-ui-tooltip'), null, 'empty title clears stale text');
      await trigger.evaluate(element => element.setAttribute('title', '再次更新'));
      await show();
      await trigger.evaluate(element => element.removeAttribute('title'));
      await hidden();
      assert.equal(await trigger.getAttribute('data-ui-tooltip'), null, 'removing title clears stale text');

      // Same microtask: mount, then set title (the order Dioxus may use).
      await page.evaluate(() => { const item = document.createElement('button'); item.id = 'dynamic'; item.textContent = 'Dynamic'; document.querySelector('#page').append(item); item.title = '动态提示'; });
      await show('#dynamic'); assert.equal(await tip.innerText(), '动态提示');
      await page.locator('#dynamic').evaluate(element => document.querySelector('#page').prepend(element));
      await away(); await show('#dynamic'); assert.equal(await tip.innerText(), '动态提示', 'moving a normalized node retains its text');
      await page.locator('#dynamic').evaluate(element => element.remove()); await hidden();

      await trigger.evaluate(element => element.title = '键盘提示');
      await page.locator('#before').click();
      await page.locator('#before').focus(); // WebKit may not focus buttons on mouse click.
      await page.keyboard.press('Tab');
      // Host-level WebKit full-keyboard-access settings may skip buttons on Tab.
      // Move focus explicitly while retaining keyboard modality in that case.
      if (await page.evaluate(() => document.activeElement.id !== 'trigger')) await trigger.focus();
      await page.locator('.ui-tooltip:not([hidden])').waitFor({ state: 'visible' });
      assert.equal(await tip.innerText(), '键盘提示');
      await page.keyboard.press('Tab');
      if (await page.evaluate(() => document.activeElement.id !== 'other')) await page.locator('#other').focus();
      await page.waitForFunction(() => document.querySelector('.ui-tooltip:not([hidden])')?.textContent === '其他提示');
      await page.keyboard.press('Escape'); await hidden();

      await show('#edge');
      const bounds = await tip.boundingBox();
      assert.ok(bounds.x >= 0 && bounds.y >= 0 && bounds.x + bounds.width <= 760 && bounds.y + bounds.height <= 580);
      // Repeated startup is idempotent, and teardown cancels all pending work.
      await page.addScriptTag({ content: script });
      assert.equal(await page.locator('.ui-tooltip').count(), 1);
      await away(); await trigger.hover();
      await page.evaluate(() => window.toolkitTooltips.destroy());
      await page.waitForTimeout(520);
      assert.equal(await page.locator('.ui-tooltip').count(), 0);
      assert.equal(await trigger.getAttribute('title'), '键盘提示');
      await page.addScriptTag({ content: script });
      await show(); assert.equal(await tip.innerText(), '键盘提示');
      assert.deepEqual(errors, []);
      console.log(`${engine}: tooltip leave/click/Escape/scroll/resize/blur/visibility/drag/cancel/route/modal/removal/title-update/focus/edge/teardown regressions passed.`);
    } finally { await browser.close(); }
  }
})().catch(error => { console.error(error); process.exit(1); });
