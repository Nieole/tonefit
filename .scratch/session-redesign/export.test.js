'use strict';
// 导出脚本自己的用例（`node --test`，见 package.json 的 `test`）。导出两趟到两个临时目录（一百多份伪 DOM
// 各起一次，一趟一分来钟），四条用例都读它们：同一份设计稿导出两次逐字节相同；库里那一份就是这一趟导出来的
// （设计稿改了没重导会红）；导出的每一份都过得了样式对照表那一关；数目对得上票面。

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('fs');
const os = require('os');
const path = require('path');

const { OUT, exportAll, SNAPSHOTS, SEQUENCES, STYLES, tree } = require('./export');

let dir, again, manifest, exported;
test.before(async () => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'design-export-'));
  again = fs.mkdtempSync(path.join(os.tmpdir(), 'design-export-again-'));
  ({ manifest } = await exportAll(dir));
  await exportAll(again);
  exported = tree(dir);
});
test.after(() => { fs.rmSync(dir, { recursive: true }); fs.rmSync(again, { recursive: true }); });

test('同一份设计稿导出两次逐字节相同', () => {
  const second = tree(again);
  assert.deepEqual([...exported.keys()], [...second.keys()]);
  for (const [file, bytes] of exported) assert.ok(bytes.equals(second.get(file)), `${file} 两趟不同`);
});

test('库里那一份就是这一趟导出来的：设计稿改了没重导，这一条红', () => {
  const committed = tree(OUT);
  for (const [file, bytes] of exported) {
    assert.ok(committed.has(file), `${file} 库里没有：重跑 npm run export 并提交`);
    assert.ok(bytes.equals(committed.get(file)), `${file} 与库里那一份不同：设计稿改了就重跑 npm run export 并提交`);
  }
  for (const file of committed.keys()) {
    if (file === 'styles.json') continue;
    assert.ok(exported.has(file), `库里多出一份 ${file}：这一趟没导出它，删掉或重跑 npm run export`);
  }
});

test('快照是 11 个场景 × 两种尺寸外加窗口太小两份；序列每一条都有名字与期望屏', () => {
  assert.equal(SNAPSHOTS.length, 11 * 2 + 2);
  const names = SEQUENCES.map((s) => s.name);
  assert.equal(new Set(names).size, names.length, '序列名撞了');
  assert.equal(manifest.sequences.length, SEQUENCES.length);
  for (const { name } of manifest.sequences) {
    for (const ext of ['text.txt', 'style.txt', 'scene.json']) assert.ok(exported.has(path.join('sequences', `${name}.${ext}`)), `${name}.${ext} 不在`);
  }
  for (const { scene, size } of manifest.snapshots) {
    for (const ext of ['text.txt', 'style.txt']) assert.ok(exported.has(path.join('snapshots', `${scene}.${size.join('x')}.${ext}`)));
    if (size[0] === 120) assert.ok(exported.has(path.join('scenes', `${scene}.json`)));
  }
});

test('样式网格里每一个代号都译得回对照表上的类名，两张网格逐行等长', () => {
  for (const [file, bytes] of exported) {
    if (!file.endsWith('.style.txt')) continue;
    const style = bytes.toString('utf8');
    const text = exported.get(file.replace(/\.style\.txt$/, '.text.txt')).toString('utf8');
    const legend = new Map();
    for (const line of style.split('\n')) {
      const m = /^# (.) = (.*)$/.exec(line);
      if (m) legend.set(m[1], m[2]);
    }
    for (const [, classes] of legend) {
      const tokens = classes.split(' ').filter(Boolean);
      const colours = tokens.filter((t) => t in STYLES.colours);
      assert.equal(colours.length, 1, `${file}：「${classes}」不是恰好一个颜色类名`);
      for (const t of tokens) assert.ok(t in STYLES.colours || t in STYLES.modifiers, `${file}：「${t}」不在对照表上`);
    }
    const styleRows = style.split('\n').filter((line) => line.startsWith('"'));
    const textRows = text.split('\n').filter((line) => line.startsWith('"'));
    assert.equal(styleRows.length, textRows.length, `${file}：两张网格行数不同`);
    styleRows.forEach((row, y) => {
      for (const code of row.slice(1, -1)) assert.ok(legend.has(code), `${file}：代号「${code}」没有说明`);
      assert.equal([...row].length, [...textRows[y]].length, `${file}：第 ${y} 行两张网格格数不同`);
    });
  }
});
