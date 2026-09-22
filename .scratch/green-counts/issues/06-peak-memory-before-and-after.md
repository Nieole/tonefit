# 06 — `two-pass-rework/12` 前后峰值内存各量一次

**What to build:** `two-pass-rework/12` 的验收说「峰值内存显著下降」，一次都没量过。同一卷、同一台机器，`two-pass-rework/12` 之前那一版与今天各量一次
进程峰值常驻（本机取峰值工作集），旧版在旁开的工作树里建（只建不跑闸门）。「显著下降」照数落实或改口。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 数只进 `docs/measurements.md`；施测条件写全（机器、提交、参数、语料、日期），宽跑写明逐卷读数落在哪儿；画质分的数取自 `tonefit --dry-run`
- [ ] 《缓存溢写》那一节添一张前后对照：卷、两版提交、参数、峰值
- [ ] `two-pass-rework/12` 票面那一格照数处理：成立就补勾并指到那一节，不成立就写明
- [ ] 旁开的工作树用完删掉
- [ ] `src/`、`tests/`、`xtask/`、`Cargo.toml` 一行都没动时不必跑闸门（`gate.md` 的例外）；动了就三条全绿

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q429 — 「峰值内存显著下降」这条验收给不出数

- **From:** 票 `two-pass-rework/12`
- **Kind:** 前提不在（这一轮量不了）
- **Where:** 票 `two-pass-rework/12` 验收第 3 条；`docs/measurements.md`《缓存溢写》
- **Why it did not block:** `_samples` 语料不在这台机器上，「峰值内存从整卷掉到几页」这类数
  这一轮**测不出来**，而 `docs/measurements.md` 是实测数字唯一的出处——推一个数写进去就是造假出处。
  验收第 3 条的**结构**那一半仍旧钉得住：参照只在有界窗口里活着，
  由 `the_window_never_holds_more_than_the_lookback` 与滚动窗口本身的形状保证。
- **What this ticket actually did:** 第 3 条**没勾**，票据的《落地记录》里写明为什么。
  没有往 `measurements.md` 里写任何数。
- **Options:** ① 照现在（记着，等有语料的那一趟）；② 拿合成夹具量一个数；③ 勾上，靠结构论证。
- **Recommend:** ①，最好并进 `two-pass-rework/16` 那一趟重跑——那一批本来就要按同一 profile 跑八卷。
  ②量出来的是夹具的数，不是素材的数；③把「显著下降」当成不用量的话，而它明写着是个量。
- **Whose call:** 拍板的人
- **处置：** 待处理。
