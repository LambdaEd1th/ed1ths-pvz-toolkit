// Test the real CSS cascade without building WASM or requiring game assets.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium, webkit } = require('playwright');

const root = path.resolve(__dirname, '../..');
const read = relative => fs.readFileSync(path.join(root, relative), 'utf8');
function cssFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const target = path.join(directory, entry.name);
    return entry.isDirectory() ? cssFiles(target) : target.endsWith('.css') ? [target] : [];
  }).sort();
}
const tokens = read('crates/ui/toolkit-ui/assets/tokens.css');
const primitives = read('crates/ui/toolkit-ui/assets/primitives.css');
const shell = read('apps/toolkit/assets/toolkit.css');
const toolStyles = cssFiles(path.join(root, 'crates/tools')).map(file => fs.readFileSync(file, 'utf8'));
const variants = [
  ['rsb', 'rsb-tool-button', 'rsb-tool-button rsb-tool-button--icon'],
  ['pam', 'pam-button', 'pam-page-icon-button'],
  ['rton', 'rton-button', 'rton-page-icon-button'],
  ['pak', 'pak-open-button', 'pak-icon-button'],
  ['dzip', 'dzip-open-button', 'dzip-icon-button'],
  ['rsb-patch', 'rsp-action-button', 'rsp-icon-button', 'rsp'],
  ['bnk', 'bnk-open-button', 'bnk-icon-button'],
  ['newton', 'newton-action-button', 'newton-icon-button'],
  ['future-tool', 'future-action', 'future-icon'], // No per-tool typography whitelist.
];
const glyph = '<svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="M2 2h12v12H2z"/></svg>';
const fixtures = variants.map(([namespace, plain, icon, prefix = namespace]) => [
  '<div class="ui-tool-surface ui-tool-surface--' + namespace + '">',
  '<div class="' + prefix + '-page-host">',
  '<section class="ui-tool-page-toolbar is-open"><div class="ui-tool-drawer-body">',
  '<div class="ui-tool-page-actions ' + prefix + '-page-actions">',
  '<button class="' + plain + '" data-case="' + namespace + '/plain">资源管理器</button>',
  '<button class="' + plain + '" disabled data-case="' + namespace + '/disabled">资源管理器</button>',
  '<button class="' + plain + ' is-active" data-case="' + namespace + '/active"><span>资源管理器</span></button>',
  '<button class="' + icon + '" title="归档目录" data-case="' + namespace + '/icon">' + glyph + '</button>',
  '<label class="' + icon + '" title="打开文件" data-case="' + namespace + '/file-label">' + glyph + '<input hidden type="file"></label>',
  '<button data-case="' + namespace + '/new-button">未来新增操作</button>',
  '<div><button class="' + plain + '" data-case="' + namespace + '/nested"><span>嵌套操作</span></button></div>',
  '</div></div></section></div></div>',
].join('')).join('');
const reference = [
  '<div id="toolbar-reference" style="font-family:var(--tk-font-ui);font-size:11px;font-weight:650;line-height:1;letter-spacing:0">参考</div>',
  '<div class="ui-tool-surface ui-tool-surface--rsb"><div class="rsb-page-host">',
  '<button id="rsb-outside" class="rsb-tool-button">非工具栏按钮</button>',
  '<input id="inherited-control" style="font-size:17px;font-weight:500">',
  '</div></div>',
  '<div class="ui-tool-surface ui-tool-surface--pam"><div class="pam-page-host">',
  '<button id="pam-outside" class="pam-button">非工具栏按钮</button>',
  '</div></div>',
].join('');

(async () => {
  const engines = (process.env.UI_TEST_BROWSERS || 'chromium,webkit').split(',');
  for (const engine of engines) {
    assert.ok(['chromium', 'webkit'].includes(engine), 'Unsupported UI_TEST_BROWSERS');
    const browser = await ({ chromium, webkit })[engine].launch({
      headless: true,
      ...(engine === 'chromium' && process.env.PLAYWRIGHT_CHROMIUM_CHANNEL
        ? { channel: process.env.PLAYWRIGHT_CHROMIUM_CHANNEL } : {}),
    });
    try {
      const page = await browser.newPage();
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      // Tool CSS is lazy-loaded. Check both orders and late shared-style insertion.
      const orders = [
        [tokens, primitives, shell, ...toolStyles],
        [tokens, primitives, shell, ...toolStyles.toReversed()],
        [shell, ...toolStyles, tokens, primitives],
      ];
      let checks = 0;
      for (const [order, styles] of orders.entries()) {
        for (const width of [1440, 390]) {
          for (const theme of ['light', 'dark']) {
            await page.setViewportSize({ width, height: 960 });
            await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
            await page.setContent('<html class="tk-theme-' + theme + '"><head>' + styles.map(css => '<style>' + css + '</style>').join('') + '</head><body>' + fixtures + reference + '</body></html>');
            const result = await page.evaluate(() => {
              const typography = (element, pseudo) => {
                const style = getComputedStyle(element, pseudo);
                return {
                  family: style.fontFamily, size: style.fontSize, weight: style.fontWeight,
                  height: style.lineHeight, spacing: style.letterSpacing, style: style.fontStyle,
                };
              };
              const expected = typography(document.querySelector('#toolbar-reference'));
              const controls = [...document.querySelectorAll('[data-case]')].flatMap(element => {
                const output = [{ name: element.dataset.case, value: typography(element) }];
                if (element.querySelector('span')) {
                  output.push({ name: element.dataset.case + '/span', value: typography(element.querySelector('span')) });
                }
                const content = getComputedStyle(element, '::after').content;
                if (element.hasAttribute('title') && !['none', 'normal', '""'].includes(content)) {
                  output.push({ name: element.dataset.case + '/::after', value: typography(element, '::after') });
                }
                return output;
              });
              return {
                expected, controls,
                rsb: typography(document.querySelector('#rsb-outside')),
                pam: typography(document.querySelector('#pam-outside')),
                custom: typography(document.querySelector('#inherited-control')),
              };
            });
            const context = engine + ', order=' + order + ', ' + width + 'px, ' + theme;
            for (const { name, value } of result.controls) {
              assert.deepEqual(value, result.expected, context + ': ' + name);
              checks++;
            }
            assert.equal(result.rsb.size, '11px', context + ': RSB base size must beat inheritance reset');
            assert.equal(result.rsb.weight, '720', context + ': RSB base weight must beat inheritance reset');
            assert.equal(result.pam.size, '12px', context + ': PAM non-toolbar size is intentional');
            assert.equal(result.pam.weight, '750', context + ': PAM non-toolbar weight is intentional');
            assert.equal(result.custom.size, '17px', context + ': explicit form-control sizing is preserved');
          }
        }
      }
      assert.deepEqual(errors, []);
      console.log(engine + ': ' + checks + ' toolbar typography comparisons passed, including pseudo-labels, inheritance resets, themes, narrow layouts and stylesheet order.');
    } finally {
      await browser.close();
    }
  }
})().catch(error => { console.error(error); process.exit(1); });
