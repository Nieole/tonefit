# 05 — 抬头说出位深走的哪条路，标定来源写两块面板

**What to build:** **位深走的哪条路**：`Report` 带上这一趟是逐页判断还是整卷统一灰阶，抬头印一行，叫法与设置那一项同一个出处——
全卷都幂等命中的那一趟也说得出自己是哪条路。

**两道标定窗口**：标定来源的类型装得下几块面板；内置的那个印成「在 boox-poke6 与 kobo-libra-2 上实测，其他屏幕未验证」。点名的那一种不变。

会话配置视图里画质判定参数那一段取自报告抬头，跟着变：照 ADR 0019 决定第 13 条先改设计稿、重导，再让实现跟上。

**Blocked by:** `design-parity/01` — 设计稿追上实现已经对了的那几处（之后才重导）

**Status:** ready-for-agent

- [ ] `render` 的用例：抬头的位深那一行两条路各一条；内置标定来源那一句说出两块面板
- [ ] 设计稿里取自库的那几句改完、`npm run export` 重导，`npm run check` 绿；相关几串比整屏且绿
- [ ] 黄金快照为本票的改动显式接受一次，diff 里只有本票那几行
- [ ] `cargo xtask gate` 三条全绿；票据的《数》记下三条各自最后一行

## 停车场结转

下面几条由停车场转来（`/settle` Q206–Q994 → `/grill-with-docs` → 本 effort 的 spec），归这张票收；上面的做法是拷问的结论，与原文的推荐不同时以上面为准。

#### Q637 — 报告抬头不说位深走的哪条路，全卷幂等命中那一趟因此说不出自己是 `--envelope` 还是逐页

- **From:** 票 `two-pass-rework/11`
- **Kind:** story 29 的一个边角
- **Where:** `src/render.rs` 的 `header`（抬头列 profile、适配方式、裁边、跨页拆分、互锁、判据三行）；`src/report.rs` 的 `Report`
  （抬头那几项都是它身上的字段）；`src/session/draw/overlay.rs` 的「这一趟的前提」覆盖层
- **Why it did not block:** 处理过的每一卷都说得出：卷级那一行 `VolumeVerdict::Envelope`／`PerPage` 只在各自那条路上出现，
  两趟混不到一起比（spec 的 story 29）。说不出的只有一种趟：**每一卷都幂等命中**——那时没有一卷有卷级判定，
  抬头又不带这个开关。那一趟本来就一页没做，读的人要的多半是「为什么全跳过」而不是「走的哪条路」。
- **What this ticket actually did:** 卷级那一行改说法（「无（默认逐页）……要卷级齐整开 --envelope」），抬头没动。
- **Options:** ① 抬头加一行「位深 逐页」／「位深 上包络」——要给 `Report` 加一个字段、`run` 填它、`header` 印它，
  会话的前提覆盖层跟着多一行（那在 `src/session/draw/`，本票不进）；② 不加，全跳过的那一趟本来无卷级判定可读；
  ③ 只在全部卷都跳过时补一句。
- **Recommend:** ①，等 tpr/01、tpr/02 合完再做——适配方式、裁边、拆分三项都在抬头上，第四个改产物的开关不在，
  是抬头那张单子漏了一项；③ 是为一种趟另开一条规则，不值。
- **Whose call:** 拍板的人（可并进 tpr/02，它正在动会话那几副的措辞）
- **处置：** 待处理。

#### Q472 — `Threshold` 的 `Display` 仍只说 boox-poke6，而今天钉住取值的是 kobo-libra-2 与本仓夹具

- **From:** `metric-recalibration/07`（阈值 5.5 → 5.123）
- **Kind:** 路过发现（屏上与报告上那一句话，兑不了 `CONTEXT.md` 给的承诺）
- **Where:** `src/profile.rs` 的 `impl Display for Threshold`——`ThresholdSource::Calibrated`
  那一支印的是「盲测标定于 boox-poke6，其余面板未复核」
- **Why it did not block:** 那句话**没有说错**：5.123 确实落在 boox-poke6 那次盲测夹出的
  [4.930, 6.022) 里，「其余面板未复核」也照旧成立。
- **What this ticket actually did:** 一个字没动那句话——改措辞超出「只改一个数」。
  但它现在**说得不全**：今天真正把取值钉死的是**另外两样**，一样都不在那句话里——
  ① 第十轮真机在 **kobo-libra-2** 上夹出的 [5.079, 5.134)（那才是窄的那一道）；
  ② 本仓那张满幅渐变夹具的 5.113（取点的下界，一个回归证人）。
  `CONTEXT.md` 的《阈值》写着「标定来源因此是它的一部分，报告里与数值一同印出」，
  《尚未确立》又写着「报告里印出的是数值**加标定来源**，读的人能自己判断它对手上那块板成不成立」——
  **读的人今天判断不了**：他看到的来源是 boox-poke6，而收窄它的是另一块面板上的判读。
- **Options:** ① 记下；② `ThresholdSource::Calibrated` 带上面板与轮次
  （`Calibrated { panel, round }`），`Display` 印全——**那是给类型加一格，不是换措辞**；
  ③ 只把 `Display` 那句话改长，来源仍是一个枚举格。
- **Recommend:** ②。这一票量出来的事实是「窗口有两道、各在各的面板上」，
  而 `ThresholdSource` 今天只装得下一道。③ 改完那句话还是编的——枚举里没有第二道的位置，
  下一次再多一道又要重写一遍。②要动对外形状，因此不在本票。
- **Whose call:** 拍板的人（`ThresholdSource` 要不要装得下不止一道窗口）
- **处置：** 待处理。
