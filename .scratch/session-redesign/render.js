#!/usr/bin/env node
'use strict';
// 在伪 DOM（jsdom）里跑设计稿自己的脚本，不需要浏览器。
//
// 眼下它做一件事：11 个场景 × 120×36、80×24、56×14 各画一遍，逐格拼出整屏，零报错才算过
// （session-redesign/01）。02 号票在它上面长出导出：快照、场景数据、交互期望屏都从 `load()`
// 交出来的那只把手上取——时钟冻住、主循环不跑，画哪一帧由调用方定。
//
//   npm install                      装 jsdom（`node_modules/` 不入库）
//   node render.js                   全画一遍，印一张表，有错退出码非零
//   node render.js --text pages 120x36
//                                    印某个场景某个尺寸的字网格（一行一屏行，两侧加引号，
//                                    宽字符后半格按 TestBackend 的读法跳过）
//   node render.js --text all        全部场景 × 尺寸的字网格

const fs = require('fs');
const path = require('path');
const { JSDOM, VirtualConsole } = require('jsdom');

/** 三种尺寸：主稿、验收线、「窗口太小」。 */
const SIZES = [[120, 36], [80, 24], [56, 14]];

/**
 * 设计稿顶层那些 `const`／`function` 里，这里与 export.js 要用到的那几个：
 * 画一帧与切尺寸、按键与鼠标的分派、推进模拟、以及导出场景数据要读的那几份假数据。
 */
const HANDLES = [
  'S', 'SCENES', 'render', 'setSize', 'term',
  'onMouse', 'onWheel', 'frame',
  'VERSION', 'SHOW', 'TARGET_H', 'CONFIG', 'PRESETS', 'PANELS', 'pagesOf', 'stepsOf', 'isolatedOutput',
];

/**
 * 把设计稿装进一个伪 DOM 里，跑完它自己的脚本与初始化，交出一只把手。
 *
 * - `performance.now` 冻在 `now`（毫秒），`clock.advance(ms)` 往前拨；转轮、回话的到期、连击键的待续都按它算；
 * - `requestAnimationFrame` 不跑：设计稿的主循环（tick + 画 + 侧栏）由调用方按需调；
 * - 页面里抛出的错误与 `console.error` 都收进 `errors`。
 */
async function load({ now = 1_000_000, file = path.join(__dirname, 'design.html') } = {}) {
  const html = fs.readFileSync(file, 'utf8');
  // 设计稿的脚本是一段 classic script：顶层 `const` 不挂在 window 上，但同一 realm 里后加的
  // classic script 看得见它们。追加一段把要用的几个交到 window 上——设计稿本身一个字不改。
  const hook = `<script>window.__design = { ${HANDLES.join(', ')} };</script>`;
  if (!html.includes('</body>')) throw new Error('design.html 里找不到 </body>');
  const errors = [];
  const virtualConsole = new VirtualConsole();
  virtualConsole.on('jsdomError', (error) => errors.push(error));
  virtualConsole.on('error', (...args) => errors.push(new Error(args.map(String).join(' '))));
  let clockNow = now;
  const dom = new JSDOM(html.replace('</body>', `${hook}\n</body>`), {
    runScripts: 'dangerously',
    virtualConsole,
    url: 'file://' + file,
    beforeParse(window) {
      window.performance.now = () => clockNow;
      window.requestAnimationFrame = () => 0;
      window.cancelAnimationFrame = () => {};
      window.addEventListener('error', (event) => errors.push(event.error || new Error(event.message)));
    },
  });
  // 初始化挂在 `document.fonts.ready`（伪 DOM 里没有，退到 Promise.resolve()）的 then 上：让它跑完。
  await new Promise((resolve) => setTimeout(resolve, 0));
  const { window } = dom;
  const design = window.__design;
  if (!design) throw new Error('设计稿的脚本没跑到底：window.__design 不在', { cause: errors[0] });
  const clock = {
    now: () => clockNow,
    set: (ms) => { clockNow = ms; },
    advance: (ms) => { clockNow += ms; },
  };
  /** 画一帧：设计稿自己的 `render`（整屏缓冲、DOM、侧栏），主循环不跑，画哪一帧由调用方定。 */
  const frame = () => design.render();
  /**
   * 让设计稿自己的主循环（`frame(now)`：推进模拟、连击键超时、画帧）跑 `ms` 毫秒，50 ms 一帧，
   * 时钟跟着走。模拟走多快照 `S.speed`（尺寸条上那几枚「模拟」chip），播放没暂停才走。
   */
  const play = (ms) => {
    for (let t = 0; t < ms; t += 50) { clockNow += Math.min(50, ms - t); design.frame(clockNow); }
  };
  /** 切尺寸：设计稿自己的 `setSize`，与尺寸那几枚 chip 同一处。 */
  const resize = (cols, rows) => design.setSize(cols, rows);
  /**
   * 按一个键，走设计稿自己的 keydown 分派（`keyName` → `onKey`）。
   * 键名照设计稿的写法：`C-w` 是 Ctrl 加 w，`Space` 是空格，其余（`j`、`Enter`、`Escape`、`Tab`、`F1`、一个汉字）原样。
   */
  const press = (key, init = {}) => {
    if (key.startsWith('C-') && key.length === 3) { init = { ctrlKey: true, ...init }; key = key.slice(2); }
    if (key === 'Space') key = ' ';
    design.term.dispatchEvent(new window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }));
  };
  /** 点一枚 chip（场景、尺寸、调色板、模拟）。 */
  const chip = (value) => {
    const button = window.document.querySelector(`.chip[data-val="${value}"]`);
    if (!button) throw new Error(`没有这一枚 chip：${value}`);
    button.click();
  };
  return { dom, window, design, errors, clock, frame, play, resize, press, chip };
}

/** 一屏的字网格：一行一屏行，两侧加引号；宽字符后半格（`null`）跳过，与 TestBackend 的读法相同。 */
function textGrid(scr) {
  return scr.cells.map((row) => `"${row.map(([ch]) => (ch === null ? '' : ch)).join('')}"`).join('\n');
}

/** 一屏的形状对不对：行数、每行的格数、没有一格是空的。 */
function checkShape(scr, cols, rows) {
  if (scr.r !== rows || scr.c !== cols) throw new Error(`屏是 ${scr.c}×${scr.r}，要的是 ${cols}×${rows}`);
  if (scr.cells.length !== rows) throw new Error(`只有 ${scr.cells.length} 行`);
  scr.cells.forEach((row, y) => {
    if (row.length !== cols) throw new Error(`第 ${y} 行有 ${row.length} 格`);
    row.forEach((cell, x) => {
      if (!Array.isArray(cell) || cell.length !== 2) throw new Error(`第 ${y} 行第 ${x} 格不是一格`);
      if (cell[0] === undefined) throw new Error(`第 ${y} 行第 ${x} 格没有字`);
    });
  });
}

/** 全部场景 × 尺寸各画一遍。回一张表：每一格是 `null`（画得出）或那个错误。 */
async function renderAll() {
  const page = await load();
  const { design, errors } = page;
  const table = [];
  for (const [name] of design.SCENES) {
    for (const [cols, rows] of SIZES) {
      const before = errors.length;
      let failure = null;
      try {
        page.chip(name);
        page.chip(`${cols}x${rows}`);
        const scr = page.frame();
        checkShape(scr, cols, rows);
        // 交互还在：挪一下、掀开全部按键再关掉，再画一帧。
        for (const key of ['j', 'k', '?', 'Escape']) page.press(key);
        page.clock.advance(100);
        checkShape(page.frame(), cols, rows);
        if (errors.length > before) failure = errors[before];
      } catch (error) {
        failure = error;
      }
      table.push({ scene: name, cols, rows, failure });
    }
  }
  return { page, table };
}

async function main(argv) {
  if (argv[0] === '--text') {
    const page = await load();
    const wanted = argv[1] === 'all' ? page.design.SCENES.map(([name]) => name) : [argv[1]];
    const sizes = argv[1] === 'all' || !argv[2] ? SIZES : [argv[2].split('x').map(Number)];
    for (const name of wanted) {
      for (const [cols, rows] of sizes) {
        page.chip(name);
        page.resize(cols, rows);
        const scr = page.frame();
        process.stdout.write(`── ${name} ${cols}×${rows} ──\n${textGrid(scr)}\n`);
      }
    }
    if (page.errors.length) {
      console.error(page.errors);
      return 1;
    }
    return 0;
  }
  const { page, table } = await renderAll();
  const width = Math.max(...table.map((row) => row.scene.length));
  for (const { scene, cols, rows, failure } of table) {
    process.stdout.write(`${scene.padEnd(width)}  ${String(cols).padStart(3)}×${String(rows).padEnd(2)}  ${failure ? `✗ ${failure.stack || failure}` : '✓'}\n`);
  }
  const failed = table.filter((row) => row.failure).length;
  process.stdout.write(`${table.length - failed}/${table.length} 画得出，${page.errors.length} 条报错\n`);
  return failed || page.errors.length ? 1 : 0;
}

module.exports = { SIZES, load, textGrid, checkShape, renderAll };

if (require.main === module) {
  main(process.argv.slice(2)).then((code) => process.exit(code), (error) => { console.error(error); process.exit(2); });
}
