'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { load, textGrid } = require('./render');

// 状态是夹具；输入与屏上的回话是接缝。每道守卫单独给一个前提，避免上游清理替下游挡住。
test('开跑先问型号，再问输出目录，最后问路径', async () => {
  const page = await load();
  try {
    for (const [model, out, says] of [
      [null, '', '先挑型号：目标尺寸与画质分都从那块面板上来，没有它跑不起来'],
      ['kobo-libra-2', '', '先填输出目录：每个卷在它下面得到一份同名副本'],
      ['kobo-libra-2', '~/转好的', '没有勾选任何路径，无法开始'],
    ]) {
      page.chip('config');
      page.design.CONFIG.find((c) => c.key === 'model').value = model;
      page.design.S.outRoot = out;
      page.design.S.named.forEach((n) => { n.on = false; });
      page.press('t');
      assert.ok(textGrid(page.frame()).includes(says), says);
      assert.equal(page.design.S.run, null, '拒绝时不应开跑');
    }
  } finally { page.dom.window.close(); }
});

test('收起预设栏使覆盖那一问作废，不借 Esc 的上游清理', async () => {
  const page = await load();
  try {
    page.chip('config');
    page.press('p');
    page.press('G');
    page.design.S.cfg.armedSave = '漫画';
    page.press('p');
    page.press('p');
    const text = textGrid(page.frame());
    assert.ok(text.includes('[⏎ → 保存为预设]'));
    assert.ok(!text.includes('再按一次 ⏎ 覆盖'));
    assert.ok(!text.includes('[⏎ → 使用]'));
  } finally { page.dom.window.close(); }
});


test('型号详情只在与预设不同时解释，没写型号不能说使用默认值', async () => {
  const page = await load();
  try {
    page.chip('config');
    page.press('h'); page.press('g'); page.press('g'); page.press('l');
    assert.ok(!textGrid(page.frame()).includes('预设「漫画」中：'));
    page.design.CONFIG.find((c) => c.key === 'model').value = 'boox-leaf2';
    let text = textGrid(page.frame());
    assert.ok(text.includes('预设「漫画」中：kobo-libra-2'));
    assert.ok(text.includes('← 与预设不同'));
    page.design.S.cfg.applied = '画集';
    text = textGrid(page.frame());
    assert.ok(text.includes('没写型号（套用时型号回到没挑）'));
    assert.ok(!text.includes('未设置（使用默认值）'));
  } finally { page.dom.window.close(); }
});


test('清点途中停止后回到路径列表，不说第零卷或开工进度', async () => {
  const page = await load();
  try {
    page.chip('survey'); page.press('s'); page.play(1000);
    const text = textGrid(page.frame());
    assert.ok(text.includes('已停止 ⋅ 清点途中停下，一卷都没开始'));
    assert.ok(text.includes('＋ 添加路径'));
    for (const old of ['处理到第 0 卷', '第 0 卷未保存', '总进度', '当前卷']) assert.ok(!text.includes(old), old);
  } finally { page.dom.window.close(); }
});


test('坏页预览过仍可跳到，目录与分区需留意而没有隔离', async () => {
  const page = await load();
  try {
    page.chip('bad-page-deciding'); page.press('s'); page.press(']'); page.press('d');
    const text = textGrid(page.frame());
    for (const word of ['❯       ! 第01卷', '▾ ! 哆啦A梦', '已分析，未写出', '问题 1/1']) assert.ok(text.includes(word), text);
    assert.match(text, /! ~\/漫画库/);
    assert.ok(!text.includes('隔离 1 卷'));
    assert.ok(!text.includes('整卷写到隔离'));
  } finally { page.dom.window.close(); }
});

test('没有需留意页时不出现需要留意', async () => {
  const page = await load();
  try {
    page.chip('retained'); page.press('a');
    const text = textGrid(page.frame());
    assert.ok(!text.includes('需要留意'));
    assert.ok(text.includes('这一卷没有需留意的页'));
  } finally { page.dom.window.close(); }
});
