#!/usr/bin/env node
'use strict';
// 从设计稿导出设计快照、场景数据与交互期望屏（session-redesign/02；spec《导出》）。
//
// 在 render.js 那只把手上长出来：伪 DOM 里跑设计稿自己的脚本，时钟冻住、主循环不跑，
// 场景推进走 `tick` 的固定步长，按键与鼠标各走设计稿自己的分派（`onKey`／`onMouse`／`onWheel`）。
// 产物落在 `tests/fixtures/design/`（跑 Rust 用例不需要 node，node 只在重新导出时要）：
//
//   styles.json                    样式对照表（手写、只有一张；这里不写它，只按它核类名）
//   manifest.json                  哪些快照、哪些序列（各带起点场景、尺寸、逐步输入）、版本号占位
//   snapshots/<场景>.<宽x高>.text.txt   字网格：一行一屏行、两侧加引号，宽字符后半格跳过
//   snapshots/<场景>.<宽x高>.style.txt  样式网格：每格一个代号，代号表在文件头（`# a = c-gray d`）
//   scenes/<场景>.json             场景数据：那一刻的语义数据，不是屏上的字
//   sequences/<名字>.text.txt / .style.txt / .scene.json
//                                  交互期望屏：走完那一屏的两张网格，外加走完那一刻的场景数据
//                                  （与起点场景一模一样的那几样写成 "unchanged"，拿起点场景那一份补上）
//
// 重新导出：
//
//   cd .scratch/session-redesign && npm install && npm run export     写回 tests/fixtures/design/
//   npm test                                                          导出两次逐字节相同等三条用例
//   node export.js --check                                            与库里那一份逐字节比，不写
//   node export.js --show <序列名或场景名> [宽x高]                      在终端里印那一屏的字网格
//
// 设计稿改了（措辞、布局、假数据）就重跑一遍导出并把产物一起提交；实现对快照只读（ADR 0019 决定第 13 条）。

const fs = require('fs');
const os = require('os');
const path = require('path');
const { load, textGrid } = require('./render');

/** 产物的去处。 */
const OUT = path.join(__dirname, '..', '..', 'tests', 'fixtures', 'design');
/** 样式对照表：只有这一张，手写在产物目录里，导出只读它。 */
const STYLES = JSON.parse(fs.readFileSync(path.join(OUT, 'styles.json'), 'utf8'));
/** 顶栏版本号在快照里的占位（与设计稿的 `VERSION` 等宽，比对时代入 crate 的版本）。 */
const VERSION_PLACEHOLDER = '#.#.#';

const MAIN = [120, 36];
const NARROW = [80, 24];
const TINY = [56, 14];

/** 要导出的快照：11 个场景 × 主稿与验收线，外加「窗口太小」两份（还没开始、转换中）。 */
const SNAPSHOTS = [
  ...['fresh', 'survey', 'running', 'deciding', 'ended', 'pages', 'envelope', 'config', 'help', 'add', 'search']
    .flatMap((scene) => [{ scene, size: MAIN }, { scene, size: NARROW }]),
  { scene: 'fresh', size: TINY },
  { scene: 'running', size: TINY },
];

// ───────────────────────── 交互序列 ─────────────────────────
//
// 每一串：起点场景、尺寸、逐步输入、走完那一屏上该看得见的一句（导出时自检，免得导出一屏不对的东西）。
// 步的写法（manifest.json 里原样给 Rust 侧）：
//   { key: 'j' }            按一个键，键名照设计稿：`C-w`、`Space`、`Enter`、`Escape`、`Tab`、`F1`、一个汉字
//   { type: '海贼' }        逐字打
//   { advance: 3 }          推进 3 秒：设计稿自己的主循环跑 3 秒（50 ms 一帧，模拟 1×，时钟跟着走），
//                           这一趟的模拟时间与会话的时钟因此各走 3 秒；结束那句回话、连击键超时都照设计稿的来
//   { click: [x, y] }       在第 x 格第 y 行单击；{ dblclick: [x, y] } 同一处连点两下（等于 ⏎）
//   { wheel: n }            滚轮滚 n 格（往下为正），一格三行；几格连滚只画一帧
//   { resize: [cols, rows] } 换尺寸（清单里 `final_size` 记走完那一屏多大）
//
// 序列名就是后面各票认领的名字：spec《交互序列》列的每一条与 03–17 各票验收点名的每一串各占一条，
// 一票要的几步分成几串各导一屏（前缀各有名字），走到哪一屏比哪一屏。
const k = (...keys) => keys.map((key) => ({ key }));
const SEQUENCES = [
  // ── 视图：`1`／`2`／`gt`／`gT`、单击视图名（spec；13：跑着时 `2` 看只读与顶栏进度；16） ──
  { name: 'running-2', scene: 'running', size: MAIN, steps: k('2'), says: '处理中' },
  { name: 'running-2-1', scene: 'running', size: MAIN, steps: k('2', '1'), says: '[自动滚动]' },
  { name: 'running-gt', scene: 'running', size: MAIN, steps: k('g', 't'), says: '设置暂时锁定' },
  { name: 'running-gt-gT', scene: 'running', size: MAIN, steps: k('g', 't', 'g', 'T'), says: '[自动滚动]' },
  { name: 'running-click-config', scene: 'running', size: MAIN, steps: [{ click: [10, 0] }], says: '设置暂时锁定' },
  { name: 'running-click-config-click-task', scene: 'running', size: MAIN, steps: [{ click: [10, 0] }, { click: [2, 0] }], says: '[自动滚动]' },

  // ── 移动：`j`／`k`、`C-d`／`C-u`、`C-f`／`C-b`、`gg`／`G`；滚轮；单击选行、双击（spec；06；16） ──
  { name: 'fresh-j', scene: 'fresh', size: MAIN, steps: k('j'), says: '银河英雄传说' },
  { name: 'fresh-k', scene: 'fresh', size: MAIN, steps: k('k'), says: '输出目录' },
  { name: 'ended-C-d', scene: 'ended', size: MAIN, steps: k('C-d'), says: '21 of 28' },
  { name: 'ended-C-d-C-u', scene: 'ended', size: MAIN, steps: k('C-d', 'C-u'), says: '第07卷' },
  { name: 'ended-C-f', scene: 'ended', size: MAIN, steps: k('C-f'), says: '28 of 28' },
  { name: 'ended-C-f-C-b', scene: 'ended', size: MAIN, steps: k('C-f', 'C-b'), says: '第07卷' },
  { name: 'ended-G', scene: 'ended', size: MAIN, steps: k('G'), says: '28 of 28' },
  { name: 'ended-G-gg', scene: 'ended', size: MAIN, steps: k('G', 'g', 'g'), says: '1 of' },
  { name: 'running-wheel-down', scene: 'running', size: MAIN, steps: [{ wheel: 1 }], says: '已暂停自动滚动' },
  { name: 'running-wheel-down-3', scene: 'running', size: MAIN, steps: [{ wheel: 3 }], says: '已暂停自动滚动' },
  { name: 'running-wheel-up', scene: 'running', size: MAIN, steps: [{ wheel: -1 }], says: '已暂停自动滚动' },
  { name: 'running-click-row', scene: 'running', size: MAIN, steps: [{ click: [10, 9] }], says: '已暂停自动滚动 ⋅ F 恢复' },
  { name: 'running-dblclick-dir', scene: 'running', size: MAIN, steps: [{ dblclick: [10, 9] }], says: '▾ ! 哆啦A梦' },

  // ── 卷列表：目录行展开、收起；卷行进每页结果；展不开的卷（08；11） ──
  { name: 'ended-h', scene: 'ended', size: MAIN, steps: k('h'), says: '❯   ▸ ✗ 集英社/海贼王' },
  { name: 'ended-h-l', scene: 'ended', size: MAIN, steps: k('h', 'l'), says: '❯   ▾ ✗ 集英社/海贼王' },
  { name: 'ended-h-Enter', scene: 'ended', size: MAIN, steps: k('h', 'Enter'), says: '❯   ▾ ✗ 集英社/海贼王' },
  { name: 'ended-h-Enter-Enter', scene: 'ended', size: MAIN, steps: k('h', 'Enter', 'Enter'), says: '❯   ▸ ✗ 集英社/海贼王' },
  { name: 'ended-l', scene: 'ended', size: MAIN, steps: k('l'), says: '[需留意的页]' },
  { name: 'ended-l-a', scene: 'ended', size: MAIN, steps: k('l', 'a'), says: '[全部页]' },
  { name: 'ended-l-a-j', scene: 'ended', size: MAIN, steps: k('l', 'a', 'j'), says: '2 of 189' },
  { name: 'ended-l-a-j-h', scene: 'ended', size: MAIN, steps: k('l', 'a', 'j', 'h'), says: '❯ ' },
  { name: 'ended-Enter', scene: 'ended', size: MAIN, steps: k('Enter'), says: '[需留意的页]' },
  { name: 'pages-h', scene: 'pages', size: MAIN, steps: k('h'), says: '第07卷' },
  { name: 'pages-Escape', scene: 'pages', size: MAIN, steps: k('Escape'), says: '第07卷' },
  { name: 'pages-a', scene: 'pages', size: MAIN, steps: k('a'), says: '[全部页]' },
  { name: 'pages-j', scene: 'pages', size: MAIN, steps: k('a', 'j', 'j'), says: '3 of 189' },
  { name: 'pages-click-page', scene: 'pages', size: MAIN, steps: [{ key: 'a' }, { click: [20, 14] }], says: '5 of 189' },
  { name: 'ended-failed-l', scene: 'ended', size: MAIN, steps: k('j', 'j', 'j', 'j', 'l'), says: '✗ 转换失败：' },
  { name: 'ended-skipped-l', scene: 'ended', size: MAIN, steps: k('g', 'g', 'l', 'j', 'l'), says: '这一卷跳过了：' },
  { name: 'running-vol-l', scene: 'running', size: MAIN, steps: k('l'), says: '还在处理：' },
  { name: 'running-queued-l', scene: 'running', size: MAIN, steps: k('j', 'l'), says: '还没轮到这一卷：' },
  { name: 'running-s-s-l', scene: 'running', size: MAIN, steps: k('s', 's', 'l'), says: '已中断：' },

  // ── 自动滚动、问题跳转、搜索（spec；09） ──
  { name: 'running-j', scene: 'running', size: MAIN, steps: k('j'), says: '已暂停自动滚动' },
  { name: 'running-j-advance', scene: 'running', size: MAIN, steps: [{ key: 'j' }, { advance: 3 }], says: '已暂停自动滚动 ⋅ F 恢复' },
  { name: 'running-j-advance-F', scene: 'running', size: MAIN, steps: [{ key: 'j' }, { advance: 3 }, { key: 'F' }], says: '自动滚动：' },
  { name: 'running-]d', scene: 'running', size: MAIN, steps: k(']', 'd'), says: '问题 4/4' },
  { name: 'running-]d-]d', scene: 'running', size: MAIN, steps: k(']', 'd', ']', 'd'), says: '问题 1/4' },
  { name: 'running-]d-]d-[d', scene: 'running', size: MAIN, steps: k(']', 'd', ']', 'd', '[', 'd'), says: '问题 4/4' },
  { name: 'ended-]d', scene: 'ended', size: MAIN, steps: k(']', 'd'), says: '问题 3/7' },
  { name: 'ended-[d', scene: 'ended', size: MAIN, steps: k('[', 'd'), says: '问题 1/' },
  { name: 'running-slash', scene: 'running', size: MAIN, steps: k('/'), says: '[⏎ → 跳到结果]' },
  { name: 'running-search-typed', scene: 'running', size: MAIN, steps: [{ key: '/' }, { type: '海贼' }], says: '/海贼' },
  { name: 'running-search-Enter', scene: 'running', size: MAIN, steps: [{ key: '/' }, { type: '海贼' }, { key: 'Enter' }], says: '搜索结果 1/1' },
  { name: 'running-search-05-Enter', scene: 'running', size: MAIN, steps: [{ key: '/' }, { type: '第05' }, { key: 'Enter' }], says: '搜索结果 3/7' },
  { name: 'running-search-n', scene: 'running', size: MAIN, steps: [{ key: '/' }, { type: '第05' }, { key: 'Enter' }, { key: 'n' }], says: '搜索结果 4/7' },
  { name: 'running-search-n-N', scene: 'running', size: MAIN, steps: [{ key: '/' }, { type: '第05' }, { key: 'Enter' }, { key: 'n' }, { key: 'N' }], says: '搜索结果 3/7' },
  { name: 'ended-search-into-collapsed', scene: 'ended', size: MAIN, steps: [{ key: '/' }, { type: '棋魂/第15' }, { key: 'Enter' }], says: { has: ['▾ ! 棋魂', '❯     ! 第15卷'] } },
  { name: 'ended-search-nothing', scene: 'ended', size: MAIN, steps: [{ key: '/' }, { type: '不存在' }, { key: 'Enter' }], says: '没有找到和「不存在」相关的卷或文件夹' },
  { name: 'search-Escape', scene: 'search', size: MAIN, steps: k('Escape'), says: '[/ → 搜索]' },
  { name: 'search-Enter-Escape', scene: 'search', size: MAIN, steps: k('Enter', 'Escape'), says: { has: ['搜索结果 1/1'], lacks: ['/海贼 ⋅ n N'] } },

  // ── 等待确认（spec；12） ──
  { name: 'deciding-v', scene: 'deciding', size: MAIN, steps: k('v'), says: '等待确认：还没写入任何文件' },
  { name: 'deciding-v-h', scene: 'deciding', size: MAIN, steps: k('v', 'h'), says: '需要确认' },
  { name: 'deciding-x', scene: 'deciding', size: MAIN, steps: k('x'), says: '写出这一卷' },
  { name: 'deciding-x-advance', scene: 'deciding', size: MAIN, steps: [{ key: 'x' }, { advance: 30 }], says: '需要确认' },
  { name: 'deciding-a', scene: 'deciding', size: MAIN, steps: k('a'), says: '全部写出：' },
  { name: 'deciding-a-advance', scene: 'deciding', size: MAIN, steps: [{ key: 'a' }, { advance: 30 }], says: '转换' },
  { name: 'deciding-s', scene: 'deciding', size: MAIN, steps: k('s'), says: '已结束预览：' },
  { name: 'deciding-2', scene: 'deciding', size: MAIN, steps: k('2'), says: '? 等待确认 ⋅' },

  // ── 停止、`q`（spec；08；10） ──
  { name: 'running-s', scene: 'running', size: MAIN, steps: k('s'), says: '正在停止：' },
  { name: 'running-s-advance', scene: 'running', size: MAIN, steps: [{ key: 's' }, { advance: 30 }], says: '已停止 ' },
  { name: 'running-s-s', scene: 'running', size: MAIN, steps: k('s', 's'), says: '已立即停止：' },
  { name: 'running-q', scene: 'running', size: MAIN, steps: k('q'), says: 'q 不会退出' },
  { name: 'survey-s', scene: 'survey', size: MAIN, steps: k('s'), says: '正在停止：' },
  { name: 'ended-q', scene: 'ended', size: MAIN, steps: k('q'), says: '退出' },
  { name: 'fresh-q', scene: 'fresh', size: MAIN, steps: k('q'), says: '退出' },

  // ── 全部按键：五个阶段各一屏；打字时 `F1`；覆盖层掀着时 `j`／`k`／`Esc`（spec；07） ──
  { name: 'fresh-help', scene: 'fresh', size: MAIN, steps: k('?'), says: '全部按键 ⋅ vim 风格' },
  { name: 'survey-help', scene: 'survey', size: MAIN, steps: k('?'), says: '全部按键 ⋅ vim 风格' },
  { name: 'deciding-help', scene: 'deciding', size: MAIN, steps: k('?'), says: '全部按键 ⋅ vim 风格' },
  { name: 'ended-help', scene: 'ended', size: MAIN, steps: k('?'), says: '全部按键 ⋅ vim 风格' },
  { name: 'help-j', scene: 'help', size: MAIN, steps: k('j'), says: '1–27 of 27' },
  { name: 'help-narrow-j-k', scene: 'help', size: NARROW, steps: k('j', 'k'), says: '1–' },
  { name: 'help-narrow-G', scene: 'help', size: NARROW, steps: k('G'), says: 'of' },
  { name: 'help-Escape', scene: 'help', size: MAIN, steps: k('Escape'), says: '[自动滚动]' },
  { name: 'help-question', scene: 'help', size: MAIN, steps: k('?'), says: '[自动滚动]' },
  { name: 'help-narrow-j', scene: 'help', size: NARROW, steps: k('j'), says: '2–' },
  { name: 'add-F1', scene: 'add', size: MAIN, steps: k('F1'), says: '全部按键 ⋅ vim 风格' },
  { name: 'add-F1-j', scene: 'add', size: MAIN, steps: k('F1', 'j'), says: '1–26 of 26' },
  { name: 'add-narrow-F1-j', scene: 'add', size: NARROW, steps: k('F1', 'j'), says: '2–' },
  { name: 'add-F1-j-k-Escape', scene: 'add', size: MAIN, steps: k('F1', 'j', 'k', 'Escape'), says: '添加路径  ~/Comics/' },
  { name: 'add-narrow-F1-j-k', scene: 'add', size: NARROW, steps: k('F1', 'j', 'k'), says: '1–' },
  { name: 'add-narrow-F1-j-k-Escape', scene: 'add', size: NARROW, steps: k('F1', 'j', 'k', 'Escape'), says: '添加路径  ~/Comics/' },

  // ── 开跑之前：`o`（补全框、`Tab` 轮换、`C-w`、找不到的路径）、`i`、空格、`dd`；结束之后 `o`（spec；06；07；10） ──
  { name: 'fresh-o', scene: 'fresh', size: MAIN, steps: k('o'), says: '添加路径  ~/' },
  { name: 'fresh-o-Tab', scene: 'fresh', size: MAIN, steps: k('o', 'Tab'), says: '可选项 ⋅ 4 项' },
  { name: 'fresh-o-Tab-Tab', scene: 'fresh', size: MAIN, steps: k('o', 'Tab', 'Tab'), says: '2 of 4' },
  { name: 'fresh-o-Tab-Tab-C-w', scene: 'fresh', size: MAIN, steps: k('o', 'Tab', 'Tab', 'C-w'), says: '添加路径  ~/' },
  { name: 'fresh-o-missing', scene: 'fresh', size: MAIN, steps: [{ key: 'o' }, { type: '没有这个' }, { key: 'Enter' }], says: '找不到「~/没有这个」' },
  { name: 'fresh-o-added', scene: 'fresh', size: MAIN, steps: [{ key: 'o' }, { type: 'Comics/火之鸟' }, { key: 'Enter' }], says: '已添加 ~/Comics/火之鸟' },
  { name: 'fresh-o-Escape', scene: 'fresh', size: MAIN, steps: k('o', 'Escape'), says: '[o → 添加路径]' },
  { name: 'fresh-i', scene: 'fresh', size: MAIN, steps: k('i'), says: '修改路径  ~/漫画库' },
  { name: 'fresh-k-i', scene: 'fresh', size: MAIN, steps: k('k', 'i'), says: '输出目录  ~/转好的/' },
  { name: 'fresh-k-i-C-w-typed', scene: 'fresh', size: MAIN, steps: [{ key: 'k' }, { key: 'i' }, { key: 'C-w' }, { type: 'Comics' }, { key: 'Enter' }], says: '输出目录 → ~/Comics' },
  { name: 'fresh-Space', scene: 'fresh', size: MAIN, steps: k('Space'), says: '[ ] ~/漫画库' },
  { name: 'fresh-Space-Space', scene: 'fresh', size: MAIN, steps: k('Space', 'Space'), says: '[x] ~/漫画库' },
  { name: 'fresh-dd', scene: 'fresh', size: MAIN, steps: k('d', 'd'), says: '已删除 ' },
  { name: 'fresh-d', scene: 'fresh', size: MAIN, steps: k('d'), says: 'd…' },
  { name: 'fresh-t', scene: 'fresh', size: MAIN, steps: k('t'), says: '预览：' },
  { name: 'fresh-x', scene: 'fresh', size: MAIN, steps: k('x'), says: '转换：' },
  { name: 'ended-o', scene: 'ended', size: MAIN, steps: k('o'), says: '返回路径列表' },
  { name: 'ended-note', scene: 'ended', size: MAIN, steps: k('C-d', 'j', 'j'), says: '[⏎ → 查看]' },
  { name: 'ended-note-Enter', scene: 'ended', size: MAIN, steps: k('C-d', 'j', 'j', 'Enter'), says: 'Esc → 关闭' },
  { name: 'ended-note-Enter-Escape', scene: 'ended', size: MAIN, steps: k('C-d', 'j', 'j', 'Enter', 'Escape'), says: '[⏎ → 查看]' },
  { name: 'ended-nonvolume-Enter', scene: 'ended', size: MAIN, steps: k('C-d', 'j', 'j', 'j', 'Enter'), says: '已忽略 3 个文件' },

  // ── 配置视图（spec；13） ──
  { name: 'config-h', scene: 'config', size: MAIN, steps: k('h'), says: '[l → 展开]' },
  { name: 'config-h-l', scene: 'config', size: MAIN, steps: k('h', 'l'), says: '[⏎ → 确定]' },
  { name: 'config-h-l-k-l', scene: 'config', size: MAIN, steps: k('h', 'l', 'k', 'l'), says: '缩放方式 → height' },
  { name: 'config-h-l-k-h', scene: 'config', size: MAIN, steps: k('h', 'l', 'k', 'h'), says: '[l → 展开]' },
  { name: 'config-Escape', scene: 'config', size: MAIN, steps: k('Escape'), says: '[l → 展开]' },
  { name: 'config-Tab', scene: 'config', size: MAIN, steps: k('Tab'), says: '[l → 展开]' },
  { name: 'config-model', scene: 'config', size: MAIN, steps: k('h', 'k', 'k', 'k', 'l'), says: '先选屏幕规格，再选型号' },
  { name: 'config-model-drill', scene: 'config', size: MAIN, steps: k('h', 'k', 'k', 'k', 'l', 'l'), says: '屏幕规格 › ' },
  { name: 'config-model-drill-h', scene: 'config', size: MAIN, steps: k('h', 'k', 'k', 'k', 'l', 'l', 'h'), says: '先选屏幕规格，再选型号' },
  { name: 'config-model-drill-j-l', scene: 'config', size: MAIN, steps: k('h', 'k', 'k', 'k', 'l', 'l', 'j', 'l'), says: '设备型号已改为 ' },
  { name: 'config-levels-i', scene: 'config', size: MAIN, steps: k('h', 'k', 'k', 'i'), says: '可见灰阶数  ' },
  { name: 'config-levels-i-typed', scene: 'config', size: MAIN, steps: [{ key: 'h' }, { key: 'k' }, { key: 'k' }, { key: 'i' }, { type: '14' }, { key: 'Enter' }], says: '可见灰阶数 → 14' },
  { name: 'config-narrow-h', scene: 'config', size: NARROW, steps: k('h'), says: '[l → 展开]' },
  { name: 'config-narrow-h-l', scene: 'config', size: NARROW, steps: k('h', 'l'), says: { has: ['当前 inside'], lacks: ['设置 ⋅ 预设会保存这两组'] } },
  { name: 'config-narrow-h-l-h', scene: 'config', size: NARROW, steps: k('h', 'l', 'h'), says: { has: ['设置 ⋅ 预设会保存这两组'], lacks: ['当前 inside'] } },
  { name: 'running-2-fit-l', scene: 'running', size: MAIN, steps: k('2', 'j', 'j', 'j', 'l'), says: '[⏎ → 查看]' },
  { name: 'running-2-fit-l-l', scene: 'running', size: MAIN, steps: k('2', 'j', 'j', 'j', 'l', 'l'), says: '设置已锁定，结束后才能修改' },
  { name: 'config-c', scene: 'config', size: MAIN, steps: k('c'), says: '已生成灰阶测试图' },
  { name: 'config-click-row', scene: 'config', size: MAIN, steps: [{ click: [10, 7] }], says: { has: ['[l → 展开]', '❯ 可见灰阶数'] } },
  { name: 'config-click-choice', scene: 'config', size: MAIN, steps: [{ click: [70, 6] }], says: '❯   height' },

  // ── 预设栏（spec；14） ──
  { name: 'config-p', scene: 'config', size: MAIN, steps: k('p'), says: '预设 ⋅ 2 个' },
  { name: 'config-p-j-Enter', scene: 'config', size: MAIN, steps: k('p', 'j', 'Enter'), says: '已使用预设「画集」' },
  { name: 'config-p-dd', scene: 'config', size: MAIN, steps: k('p', 'd', 'd'), says: '再按一次 dd 删除「漫画」' },
  { name: 'config-p-dd-dd', scene: 'config', size: MAIN, steps: k('p', 'd', 'd', 'd', 'd'), says: '已删除预设「漫画」' },
  { name: 'config-p-save', scene: 'config', size: MAIN, steps: k('p', 'G', 'Enter'), says: '保存为预设，名称  ' },
  { name: 'config-p-save-named', scene: 'config', size: MAIN, steps: [{ key: 'p' }, { key: 'G' }, { key: 'Enter' }, { type: '插图' }, { key: 'Enter' }], says: '已保存预设「插图」' },
  { name: 'config-p-p', scene: 'config', size: MAIN, steps: k('p', 'p'), says: '[l → 展开]' },
  { name: 'config-p-h', scene: 'config', size: MAIN, steps: k('p', 'h'), says: '[l → 展开]' },
  { name: 'config-p-click-preset', scene: 'config', size: MAIN, steps: [{ key: 'p' }, { click: [70, 6] }], says: '❯   画集' },
  { name: 'running-2-p-Enter', scene: 'running', size: MAIN, steps: k('2', 'p', 'Enter'), says: '正在转换，设置已锁定' },

  // ── 窗口缩到 60×16 以下再放大（spec） ──
  { name: 'running-shrink', scene: 'running', size: MAIN, steps: [{ resize: TINY }], says: '至少需要 60x16' },
  { name: 'running-shrink-grow', scene: 'running', size: MAIN, steps: [{ resize: TINY }, { resize: MAIN }], says: '[自动滚动]' },
];

// ───────────────────────── 两张网格 ─────────────────────────

/** 一格的类名拆成颜色一个、修饰一组：`''` 与 `'d'` 都是终端默认色。核每一个记号都在对照表上。 */
function styleOf(classes) {
  const tokens = classes.split(' ').filter(Boolean);
  const colours = tokens.filter((t) => t in STYLES.colours);
  const modifiers = tokens.filter((t) => t in STYLES.modifiers);
  const unknown = tokens.filter((t) => !(t in STYLES.colours) && !(t in STYLES.modifiers));
  if (unknown.length) throw new Error(`样式类名「${classes}」里的「${unknown.join(' ')}」不在 styles.json 上`);
  if (colours.length > 1) throw new Error(`样式类名「${classes}」里有两个颜色`);
  const order = Object.keys(STYLES.modifiers);
  return [colours[0] || 'c-fg', ...[...new Set(modifiers)].sort((a, b) => order.indexOf(a) - order.indexOf(b))].join(' ');
}

const CODES = '.abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';

/**
 * 一屏的样式网格：每格一个代号（`.` 恒是终端默认色、无修饰），宽字符后半格跳过，
 * 代号表在文件头（`# a = c-gray d`），按首次出现的次序编号。
 */
function styleGrid(scr) {
  const legend = new Map([['c-fg', '.']]);
  const rows = scr.cells.map((row) => `"${row.map(([ch, st]) => {
    if (ch === null) return '';
    const style = styleOf(st);
    if (!legend.has(style)) {
      if (legend.size >= CODES.length) throw new Error('一屏上的样式种类超过了代号表');
      legend.set(style, CODES[legend.size]);
    }
    return legend.get(style);
  }).join('')}"`);
  const head = [...legend.entries()].map(([style, code]) => `# ${code} = ${style}`);
  return `${head.join('\n')}\n${rows.join('\n')}\n`;
}

/** 顶栏那个版本号换成占位：与 `VERSION` 等宽，比对时代入 crate 的版本。 */
function withVersionPlaceholder(scr, version) {
  if (version.length !== VERSION_PLACEHOLDER.length) throw new Error(`设计稿的版本号 ${version} 与占位 ${VERSION_PLACEHOLDER} 不等宽`);
  const row = scr.cells[0];
  const text = row.map(([ch]) => (ch === null ? '' : ch)).join('');
  const at = text.indexOf(`tonefit ${version}`);
  if (at < 0) return scr;
  // 字网格里的下标要换回格下标：前面每一个宽字符占两格
  let col = 0, seen = 0;
  while (seen < at + 'tonefit '.length) { if (row[col][0] !== null) seen++; col++; }
  for (let i = 0; i < version.length; i++) row[col + i] = [VERSION_PLACEHOLDER[i], row[col + i][1]];
  return scr;
}

// ───────────────────────── 场景数据 ─────────────────────────

/** 设计稿的理由种类（`page.why`）→ 库 `Reason` 的变体名。 */
const REASONS = { lowest: 'lowest-within-threshold', envelope: 'volume-envelope', outlier: 'outlier', gate: 'outside-the-gate' };

/**
 * 目录与卷在盘上的路径。分区底下的目录在那条处理路径底下；顶格目录行要么就是那条处理路径
 * （它只展开成一个目录），要么是几个点名的压缩包按父目录合成的那一行；卷根是目录加卷名，
 * 点名的压缩包带回 `.cbz`。设计稿自己不记这些（`parentHint` 对分区底下的目录记的是处理路径的父目录），
 * 这里按树的结构算出来。
 */
function roots(design) {
  const { S } = design;
  const dirs = new Map();
  for (const e of S.run.tree.entries) {
    if (e.type === 'section') for (const d of e.dirs) dirs.set(d.id, `${e.path}/${d.label}`);
    else dirs.set(e.dir.id, `${e.dir.parentHint}${e.dir.label}`);
  }
  const archives = new Set(S.named.filter((n) => n.kind === '压缩包').map((n) => n.path));
  const dirRoot = (d) => dirs.get(d.id);
  const rootOf = (v) => { const root = `${dirRoot(v.dir)}/${v.name}`; return archives.has(`${root}.cbz`) ? `${root}.cbz` : root; };
  return { dirRoot, rootOf };
}

/** 数四舍五入到千分位：模拟推进的浮点尾数不进产物。 */
const num = (x) => Math.round(x * 1000) / 1000;

/** 设计稿的设置键 → 预设文件里那一项的名字（`src/preset.rs` 的 `OnDiskDevice`／`OnDiskTaste`，kebab-case）。 */
const SETTING_KEYS = {
  model: 'profile', levels: 'gray-levels', threshold: 'threshold',
  fit: 'fit', crop: 'crop', split: 'split', splitAt: 'split-threshold', order: 'reading-order', filter: 'filter',
  white: 'white-align-limit', depth: 'bit-depth', dither: 'dither', envelope: 'envelope', cache: 'cache-budget', io: 'io-mode',
};
/** 设计稿里一项的取值 → 预设文件里那一项的类型：开关是布尔，数是数，灰阶档位是位数，其余是命令行上那个写法。 */
function settingValue(key, value) {
  if (value == null) return null;
  const on = { '裁': true, '不裁': false, '拆': true, '不拆': false, '开': true, '关': false };
  if (['crop', 'split', 'envelope'].includes(key)) { if (!(value in on)) throw new Error(`${key} 的取值认不出：${value}`); return on[value]; }
  if (key === 'depth') { const m = /^(\d)bit$/.exec(value); if (!m) throw new Error(`灰阶档位认不出：${value}`); return Number(m[1]); }
  if (['levels', 'threshold', 'splitAt', 'white'].includes(key)) { const n = Number(value); if (!Number.isFinite(n)) throw new Error(`${key} 的取值不是数：${value}`); return n; }
  return value;
}
const settingsOf = (pairs) => Object.fromEntries(pairs.map(([key, value]) => [SETTING_KEYS[key], settingValue(key, value)]));

/** 一页那一刻的语义字段（设计稿页上数在前、字在后：读的是数），不是屏上的字。 */
function pageData(design, p) {
  const out = { name: p.name, output_size: [p.width, design.TARGET_H], kinds: p.kinds.slice() };
  if (p.failure) {
    out.failure = p.failure;
    return out;
  }
  // 源页高按设计稿的总缩放比反推（设计稿只有这一个数）；预缩几档、残差多少由库自己算
  out.source_height = Math.round(p.ratio * design.TARGET_H);
  out.verdict = p.verdict;
  if (!(p.why in REASONS)) throw new Error(`理由的种类认不出：${p.why}`);
  out.reason = REASONS[p.why];
  out.score = p.score;
  if (p.salvaged != null) out.salvaged_percent = p.salvaged;
  out.driver = !!p.driver;
  return out;
}

/** 一条备注（无法访问的地方一处一条、非漫画文件一组一条）。 */
const noteData = (n) => ({ kind: n.kind === 'unreach' ? 'unreachable' : 'non-volume', entries: n.entries.map(([path, reason]) => ({ path, reason })) });

/** 光标停在哪一行：按身份说，不按屏上第几行。 */
function cursorData(design, key, { dirRoot, rootOf }) {
  const S = design.S;
  if (!key) return null;
  if (key === 'out' || key === 'add') return { kind: key };
  if (key.startsWith('np:')) return { kind: 'path', path: key.slice(3) };
  if (key.startsWith('n:')) return { kind: 'note', what: key.slice(2) };
  const r = S.run;
  if (!r) return { kind: 'unknown', key };
  if (key.startsWith('d:')) {
    const d = r.tree.entries.flatMap((e) => (e.type === 'section' ? e.dirs : [e.dir])).find((d) => 'd:' + d.id === key);
    return d ? { kind: 'directory', root: dirRoot(d) } : { kind: 'unknown', key };
  }
  if (key.startsWith('v:')) {
    const v = r.tree.vols.find((v) => 'v:' + v.id === key);
    return v ? { kind: 'volume', root: rootOf(v) } : { kind: 'unknown', key };
  }
  return { kind: 'unknown', key };
}

/** 这一刻的场景数据（spec《导出》：语义字段，不是屏上的字）。 */
function sceneData(page) {
  const { design, clock } = page;
  const { S, CONFIG, PRESETS, PANELS, SHOW, pagesOf, stepsOf, isolatedOutput } = design;
  const r = S.run;
  const cfgIndex = (i) => (CONFIG[i] && CONFIG[i].key) || null;
  const settings = settingsOf(CONFIG.filter((c) => c.key && c.kind !== 'info').map((c) => [c.key, c.value ?? null]));
  const { dirRoot, rootOf } = r ? roots(design) : { dirRoot: () => null, rootOf: () => null };
  const rightOf = () => {
    const c = CONFIG[S.cfg.cursor];
    if (!c) return null;
    if (c.kind === 'ring') return S.cfg.right === 0 ? null : c.options[S.cfg.right - 1] ?? null;
    if (c.kind === 'model') return S.cfg.drill == null ? (PANELS[S.cfg.right] || [null])[0] : (PANELS[S.cfg.drill][1][S.cfg.right] ?? null);
    return null;
  };
  const data = {
    scene: S.scene,
    now_ms: clock.now(),
    time_multiplier: SHOW,
    output: S.outRoot,
    paths: S.named.map((n) => ({ path: n.path, kind: n.kind === '压缩包' ? 'archive' : 'directory', checked: n.on })),
    settings,
    applied_preset: S.cfg.applied,
    presets: PRESETS.map((p) => ({ name: p.name, says: settingsOf(Object.entries(p.says)) })),
    run: null,
    session: {
      view: S.view,
      cursor: cursorData(design, S.curKey, { dirRoot, rootOf }),
      expanded: r ? r.tree.entries.flatMap((e) => (e.type === 'section' ? e.dirs : [e.dir])).filter((d) => S.expanded.has(d.id)).map(dirRoot) : [],
      follow: S.follow,
      pages: S.pages ? { volume: rootOf(S.pages.vol), cursor: S.pages.cursor, all: S.pages.all } : null,
      overlay: S.overlay ? (S.overlay.kind === 'help' ? { kind: 'help', from: S.overlay.from } : { kind: 'note', note: noteData(S.overlay.note) }) : null,
      input: S.input ? { kind: S.input.kind, prompt: S.input.prompt, buffer: S.input.buf, candidates: S.input.cands ? S.input.cands.map((c) => c.name + (c.dir ? '/' : '')) : null, candidate: S.input.cands ? S.input.ci : null } : null,
      search: S.search ? { query: S.search.q } : null,
      toast: S.toast && S.toast.until > clock.now() ? S.toast.segs.map(([t]) => t).join('') : null,
      pending: S.pending,
      config: { pane: S.cfg.pane, cursor: cfgIndex(S.cfg.cursor), choice: rightOf(), choice_index: S.cfg.right, drill: S.cfg.drill == null ? null : PANELS[S.cfg.drill][0], presets: S.cfg.presets, preset_cursor: S.cfg.pcursor, armed_delete: S.cfg.armedDelete == null ? null : (PRESETS[S.cfg.armedDelete] || {}).name ?? null },
    },
  };
  if (!r) return data;
  data.run = {
    mode: r.mode === 'exec' ? 'process' : 'dry-run',
    stage: r.stage,
    outcome: r.outcome,
    envelope: r.envelope,
    elapsed_s: num(r.elapsed * SHOW),
    steps: num(r.steps),
    total_steps: r.total,
    current: r.cur >= 0 && r.cur < r.tree.vols.length ? rootOf(r.tree.vols[r.cur]) : null,
    stop_level: r.latch,
    for_the_rest: r.forTheRest,
    wrote: r.wrote,
    survey: {
      entries: r.tree.entries.map((e) => (e.type === 'section'
        ? { kind: 'section', path: e.path, directories: e.dirs.map(dirRoot), notes: e.notes.map(noteData) }
        : { kind: 'directory', root: dirRoot(e.dir), notes: (e.notes || []).map(noteData) })),
      volumes: r.tree.vols.map((v) => ({ root: rootOf(v), directory: dirRoot(v.dir), name: v.name, source_pages: v.pages, steps: stepsOf(v) })),
      unchecked: r.tree.unchecked,
    },
    // 每卷此刻怎么样。逐页结果只给**屏上开着的那一卷**整份（一趟 84 卷 × 近两百页，整份摆出来一个场景就是
    // 六兆字节）；其余各卷给灰阶分布与要紧的那几页——屏上从它们身上能读到的只有这两样，
    // 夹具照分布补齐不要紧的页就是（05 号票）。
    volumes: r.tree.vols.map((v, i) => {
      const s = r.vs[i];
      const out = { root: rootOf(v), state: s.state, pass: s.pass, done: num(s.done), elapsed_s: num(s.elapsed * SHOW), tally: s.tally ? s.tally.map(([k, n]) => [k, n]) : null };
      if (s.state === 'failed') out.failure = v.plan.volFail;
      if (s.state === 'isolated') out.isolated_output = isolatedOutput(v);
      if (s.tally) {
        const pages = pagesOf(v);
        out.page_count = pages.length;
        if (S.pages && S.pages.vol === v) out.pages = pages.map((p) => pageData(design, p));
        else out.notable_pages = pages.filter((p) => p.kinds.length).map((p) => pageData(design, p));
      }
      return out;
    }),
  };
  return data;
}

// ───────────────────────── 回放与落盘 ─────────────────────────

/** `says` 写成一句或 `{ has, lacks }`：走完那一屏上该看得见的几句、不该再有的几句。 */
const wanted = (says) => (typeof says === 'string' ? { has: [says], lacks: [] } : { has: says.has || [], lacks: says.lacks || [] });

/** 一步一步走完一串输入；每一步之后画一帧（鼠标命中的是上一帧交出的区域）。 */
function replay(page, steps) {
  const { design } = page;
  for (const step of steps) {
    if ('key' in step) page.press(step.key);
    else if ('type' in step) for (const ch of step.type) page.press(ch);
    else if ('advance' in step) page.play(step.advance * 1000);
    else if ('click' in step) design.onMouse(step.click[0], step.click[1], 1);
    else if ('dblclick' in step) { design.onMouse(step.dblclick[0], step.dblclick[1], 1); design.onMouse(step.dblclick[0], step.dblclick[1], 2); }
    else if ('wheel' in step) for (let i = 0; i < Math.abs(step.wheel); i++) design.onWheel(Math.sign(step.wheel));
    else if ('resize' in step) page.resize(step.resize[0], step.resize[1]);
    else throw new Error(`不认识这一步：${JSON.stringify(step)}`);
    page.frame();
  }
}

/** 摆好一个场景与尺寸，画一帧（每一份都从新装的伪 DOM 起，上一份留下的状态一点都带不过来）。 */
async function stage(scene, size) {
  const page = await load();
  page.chip(scene);
  page.chip('1');   // 模拟 1×：「推进 N 秒」就是模拟走 N 秒
  page.resize(size[0], size[1]);
  page.frame();
  return page;
}

const grids = (page) => {
  const scr = withVersionPlaceholder(page.frame(), page.design.VERSION);
  return { text: `${textGrid(scr)}\n`, style: styleGrid(scr) };
};

const write = (file, content) => { fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, content); };
const json = (value) => `${JSON.stringify(value, null, 2)}\n`;

/** 全部导出到一个目录里。回 manifest。 */
async function exportAll(out = OUT) {
  const manifest = {
    exported_by: '.scratch/session-redesign/export.js（重新导出：cd .scratch/session-redesign && npm run export）',
    styles: 'styles.json',
    version_placeholder: VERSION_PLACEHOLDER,
    snapshots: [],
    sequences: [],
  };
  for (const { scene, size } of SNAPSHOTS) {
    const page = await stage(scene, size);
    const { text, style } = grids(page);
    const base = path.join(out, 'snapshots', `${scene}.${size.join('x')}`);
    write(`${base}.text.txt`, text);
    write(`${base}.style.txt`, style);
    if (size === MAIN) write(path.join(out, 'scenes', `${scene}.json`), json(sceneData(page)));
    if (page.errors.length) throw new Error(`${scene} ${size.join('x')} 画的时候报错：${page.errors[0].stack || page.errors[0]}`);
    manifest.snapshots.push({ scene, size, text: `snapshots/${scene}.${size.join('x')}.text.txt`, style: `snapshots/${scene}.${size.join('x')}.style.txt`, ...(size === MAIN ? { data: `scenes/${scene}.json` } : {}) });
  }
  const baseline = new Map();
  for (const seq of SEQUENCES) {
    if (!baseline.has(seq.scene)) baseline.set(seq.scene, sceneData(await stage(seq.scene, MAIN)));
    const page = await stage(seq.scene, seq.size);
    replay(page, seq.steps);
    const { text, style } = grids(page);
    for (const said of wanted(seq.says).has) if (!text.includes(said)) throw new Error(`序列 ${seq.name} 走完的那一屏上没有「${said}」：\n${text}`);
    for (const said of wanted(seq.says).lacks) if (text.includes(said)) throw new Error(`序列 ${seq.name} 走完的那一屏上不该还有「${said}」：\n${text}`);
    if (page.errors.length) throw new Error(`序列 ${seq.name} 回放时报错：${page.errors[0].stack || page.errors[0]}`);
    const base = path.join(out, 'sequences', seq.name);
    write(`${base}.text.txt`, text);
    write(`${base}.style.txt`, style);
    write(`${base}.scene.json`, json(unchangedElided(sceneData(page), baseline.get(seq.scene))));
    // `size` 是起点场景摆在多大的屏上，`final_size` 是走完那一屏多大：只有换尺寸那几串两者不同
    manifest.sequences.push({ name: seq.name, scene: seq.scene, size: seq.size, final_size: page.design.S.size.slice(), steps: seq.steps, says: seq.says, text: `sequences/${seq.name}.text.txt`, style: `sequences/${seq.name}.style.txt`, data: `sequences/${seq.name}.scene.json` });
  }
  write(path.join(out, 'manifest.json'), json(manifest));
  return { manifest };
}

/**
 * 序列走完那一刻的场景数据里，与起点场景一模一样的那几样（多半是整趟的 `run`、路径、设置）
 * 写成 `"unchanged"`：读的人拿起点场景那一份补上。不这么做一串序列就是几十千字节、一百多串就是几兆。
 */
function unchangedElided(data, base) {
  const out = {};
  for (const [key, value] of Object.entries(data)) {
    out[key] = key !== 'scene' && key !== 'session' && JSON.stringify(value) === JSON.stringify(base[key]) ? 'unchanged' : value;
  }
  return out;
}

/** 一棵目录里全部文件：相对路径 → 字节（按名字排好）。 */
function tree(root) {
  const out = new Map();
  const walk = (dir) => {
    for (const name of fs.readdirSync(dir).sort()) {
      const full = path.join(dir, name);
      if (fs.statSync(full).isDirectory()) walk(full);
      else out.set(path.relative(root, full), fs.readFileSync(full));
    }
  };
  walk(root);
  return out;
}

/** 与库里那一份逐字节比：不一样的文件名列出来。 */
async function check() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'design-export-'));
  await exportAll(dir);
  const mine = tree(dir), theirs = tree(OUT);
  const differ = [...mine.keys()].filter((file) => !theirs.has(file) || !mine.get(file).equals(theirs.get(file)));
  fs.rmSync(dir, { recursive: true });
  return differ;
}

async function main(argv) {
  if (argv[0] === '--check') {
    const differ = await check();
    for (const file of differ) process.stdout.write(`不同：${file}\n`);
    process.stdout.write(differ.length ? `${differ.length} 个文件与库里那一份不同，重跑 npm run export\n` : '与库里那一份逐字节相同\n');
    return differ.length ? 1 : 0;
  }
  if (argv[0] === '--show') {
    const seq = SEQUENCES.find((s) => s.name === argv[1]);
    const size = argv[2] ? argv[2].split('x').map(Number) : seq ? seq.size : MAIN;
    const page = await stage(seq ? seq.scene : argv[1], size);
    if (seq) replay(page, seq.steps);
    process.stdout.write(grids(page).text);
    if (page.errors.length) { console.error(page.errors); return 1; }
    return 0;
  }
  const { manifest } = await exportAll(OUT);
  process.stdout.write(`导出到 ${path.relative(process.cwd(), OUT)}：${manifest.snapshots.length} 份快照、${manifest.sequences.length} 串期望屏\n`);
  return 0;
}

module.exports = { OUT, STYLES, SNAPSHOTS, SEQUENCES, VERSION_PLACEHOLDER, exportAll, sceneData, styleGrid, replay, stage, tree };

if (require.main === module) {
  main(process.argv.slice(2)).then((code) => process.exit(code), (error) => { console.error(error); process.exit(2); });
}
