# 09 — 语义色补三处

**What to build:** 四种语义色（平常／注意／出事／不要紧）还差三处：

- **报告末尾那几小结整段一种色**，而六小结分属三档语义。
  把它拆成**一小结一段**——仍在措辞那一层出，只是粒度细了，与「行与格」同一条路，
  **不新增第二个出处**；
- **过期副本在四档里一档都不占**，给它「注意」；
- **屏底那一句没有轻重之分**：按 `x` 跑不起来的那句与「存下了一份预设」同色同位，
  一条拒绝读起来像一次成功。屏底那一句今天是裸文字，要给它挂一档语义。

**颜色不是唯一载体**这条照旧：每一处上色的地方旁边都另有一个字或一个记号，
`NO_COLOR` 那一张与上色那一张的**文字逐格相同**。

收停车场的 **Q155**、**Q156**、**Q157**。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 报告末尾按小结分档上色，措辞仍只有一处出处
- [x] 过期副本进「注意」档，旁边有字
- [x] 屏底那一句分得出轻重，拒绝与「存下了」不同色
- [x] `NO_COLOR` 那一张与上色那一张文字逐格相同
- [x] 挑颜色仍只有一处，画法各处按语义要色
- [x] 三条闸门全绿

> **六条全做到，没有保留。** 三处**判断**（不是保留，但要写明读法）：
>
> - **隔离那一小结给的是「注意」，不是「出事」。** 那一句里数着两件事
>   （「隔离 N 卷 · 失败 M 页」），而 `Tone` 那条现成的规矩说「一行上摆着好几件事时
>   取最重的那一种」。三条对上了才这么定：`CONTEXT.md` 的《语义色》把「隔离」明写在
>   「注意」一档、卷表上隔离那一行给的也是它、本票收的 Q155 自己就写着
>   「隔离也是『注意』」。算式与另一条路记在新的 **Q197**。
> - **屏底那一句挂的是状态机自己的三档（`NoticeKind`），不是 `Tone`。**
>   `Tone` 住在画法那一层，而那一层整个在 `tui` 特性后面（`#[cfg(feature = "tui")] mod draw;`）
>   ——说出那句话的 `state.rs` 在特性前面，挂不上它。折成语义与记号由画法那一头一个
>   `match` 一起定（`footer::marked`）。代价与出路记在新的 **Q198**。
> - **`CONTEXT.md` 一个字没改。**《语义色》三处要跟着改，写成一条补在 Q163 上
>   （第 13 条），归 28 号票。
>
> **评审提的五条都收了。** 两条是真的：`tail_row` 从前用 `other =>` 收尾，
> 第七小结要到收场那一帧才恐慌（那一刻终端在备用屏、raw 模式里）——此刻别的行
> **逐个列着**，第七小结是编译错误；`NO_COLOR` 那一条新加的「确实上了色」问的是
> 「这一屏上有没有红的」，屏上别处有一格红就照样绿——此刻按文字定位到**屏底那一行**。
> 另三条是文档与用例：`plain::tail` 的文档链接指进了 `tui` 特性后面的画法层（改成
> 反引号，`cargo doc --no-default-features` 那一趟因此也是 15 条）、`render.rs` 的
> `tail` 文档把「哪一小结挂哪一档」又抄了一遍（那张表只该有 `tail_row` 一处——
> 这一层按约束③本就不该知道颜色，Q197 真改了它会留在原地撒谎）、
> 屏底那个记号吃掉两格而三条用例都只在宽屏上问（补了一条 40 格的：折出来的那几行
> 拼回去那一句原样都在）。

## 落地记录

### 一、报告末尾那几小结拆成一小结一段，各上各的色（Q155）

`tail` 从 `String` 变成 `Vec<Row>`——**一小结一行**，次序就是它原来排的那个次序
（轻的在前、重的压尾），空着的小结**一行都不出**：

```rust
pub fn tail(report: &Report) -> Vec<Row> {
    [
        (RowKind::NonVolumeTail, non_volume_tail(report)),
        (RowKind::OverflowTail, overflow_tail(report)),
        (RowKind::BackstopTail, backstop_tail(report)),
        (RowKind::SalvageTail, salvage_tail(report)),
        (RowKind::IsolationTail, isolation_tail(report)),
        (RowKind::FailedVolumeTail, failed_volume_tail(report)),
    ]
    .into_iter()
    .filter(|(_, said)| !said.is_empty())
    .map(|(kind, said)| sentence_row(kind, said))
    .collect()
}
```

**六小结那几段一个字没动**——`non_volume_tail` 到 `failed_volume_tail` 六个函数一格没改。
拆的是**粒度**，走的是「行与格」那条现成的路（ADR 0016：措辞一处、排版两副）：
措辞照旧只在 `render` 出，一小结整段装在成句那一格（`Field::Sentence`）里。

**拼回一段的规矩住在 `plain` 那一头**，不在措辞那一层——`tail` 因此只有一种输出：

```rust
pub fn tail(report: &Report) -> String {
    text(&super::tail(report))
}
```

`plain::line` 六种摆法相同（那一整段本来就排好了版，这一副一格都不动它），
而**分成六种**：会话那一副要照它们各自的语义上色，认字符串是认不出来的。

会话那一头照**它是哪一小结**（`RowKind`）要一种语义（`src/session/draw/report.rs` 的 `tail_row`）：

| 小结 | 语义 | 凭什么 |
|---|---|---|
| 非卷文件 | 平常 | 它**连失败都不是**，退出码一格不动（`CONTEXT.md` 的《失败》） |
| 输出宽超过面板 · 兜底上界 · 部分救回 · 隔离 | 注意 | 四样在《语义色》那张表的「注意」一档里逐条列着 |
| 卷级失败 | 出事 | 卷根本没交出来 |

**六种一条不落、不留 `_`**：末尾多一小结该挂哪一档是个要当场拿的主意。
**颜色不是唯一载体**这条不必另想办法——六小结各自以自己那个词开头
（「非卷文件 3 个」「兜底上界 1 页」「隔离 1 卷」「卷级失败 1 卷」……），这一层一个空格都没添。

**屏上的字一格没变**：`wrap::fold` 按换行切，而每一小结那一段本来就以换行收尾——
`fold(甲 + 乙) == fold(甲) ++ fold(乙)`。用例逐行比着问的正是这一条。

### 二、过期副本进「注意」档（Q156）

`under` 那一处两种收成一档：

```rust
RowKind::Superseded | RowKind::Salvaged => Some((row, Tone::Caution)),
```

理由写在那一处的文档上（见《停车场结转》Q156 的处置）。**旁边那个字本来就在**：
那一句以「过期副本」四个字开头，措辞出自 `render`，这一层没添一个字——
`NO_COLOR` 那一趟因此一个字都不丢。

### 三、屏底那一句挂一档语义（Q157）

`notice` 从裸 `String` 变成一个类型，**两个出口都跟着改**：

| 出口 | 从前 | 此刻 |
|---|---|---|
| 写进去（`Session::says`，本模块每一句话的唯一出口） | `Option<String>` | `Option<Notice>` |
| 读出来（`Session::notice`） | `Option<&str>` | `Option<&Notice>`（`said()` 给字，`kind()` 给轻重） |

九处说话的地方各自认下自己那一种：`complain` 与解析不过、补全不着那两处是**没做成**；
`name_is_taken` 与 `ask_before_erasing` 是**先问一句**（那一句里摆着「再按一次」，
而按下去没有撤销）；`saved` / `erased` / `took` / `charted` 是**做成了**。

**轻重那一档是状态机自己的三档**（`NoticeKind`），不是 `Tone`——`Tone` 住在画法那一层，
而那一层整个在 `tui` 特性后面（`src/session.rs`：`#[cfg(feature = "tui")] mod draw;`），
说出那句话的 `state.rs` 在特性前面、闸门 2 那一趟照编不误，挂不上它（新记的 Q198）。
折成语义由画法那一头定，与「一行折成一种语义」同一条路。

**记号与语义绑成一对**，与卷表那几行同一条（`table::Mark`）：

```rust
fn marked(notice: &Notice) -> Painted {
    let (glyph, tone) = match notice.kind() {
        NoticeKind::Refused => ('✗', Tone::Trouble),
        NoticeKind::Asked => ('!', Tone::Caution),
        NoticeKind::Done => ('✓', Tone::Plain),
    };
    Painted::new(format!("{glyph} {}", notice.said()), tone)
}
```

一个 `match` 同时定记号与语义，添一种不配语义（或反过来）根本编不过去。
三个字形逐个过得了 `crate::wrap::width_is_stable` 那一关，与卷表那四个记号同一批。
**措辞一个字都没动**：记号是这一层添的排版。

`footer` 因此从「一摞 `String` 最后统一 `Line::from`」改成「按键那几行是 `String`、
要说的那句话是画好的 `Line`」——让位的次序、垫空行的规矩、`room` 怎么算全都一格没动。

### 四、`NO_COLOR` 那一条跟着长了一段

`the_same_screen_reads_the_same_with_or_without_colour` 从前只钉**主区**
（`main_snapshot` 画的是 `main_pane`），而屏底那一格在主区外面——屏底那一句上色之后，
屏上没有一处问得出「文字有没有跟着变」。此刻它多问一屏：一个说着拒绝的会话，
整屏画两遍（按住上色、按住不上色）比字，再问一句「上色那一张确实有红的」。

### 五、`paint.rs` 的模块文档跟着改口

那张表的「用在哪」一列添了三格（末尾那几小结、过期副本、屏底那一句），
「屏上按语义要色的地方」从**四处**改成**八处**——从前那句漏了逐页表与摆在一卷底下那两句，
这一次一并补齐。「颜色不是唯一载体」那一段把新添的三种载体点了出来。
**`CONTEXT.md` 一个字没改**：《语义色》要跟着改的三处写成一条补在停车场 Q163 上（第 13 条），
归 28 号票。

### 数

三条闸门跑满，三条都绿，一条失败都没有。

| 闸门 | 最后一行 | 合计 |
|---|---|---|
| `cargo test` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **781 通过 0 失败**；lib **210** / bin **318** |
| `cargo test --no-default-features` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（Doc-tests 那一格） | **662 通过 0 失败**；lib **210** / bin **199** |
| `cargo check --features profiling` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.82s`` | 干净，一条告警都没有 |

**闸门 1 从 777 涨到 781**（+4），**闸门 2 从 661 涨到 662**（+1）——lib 两趟都是 **210**，
一格没动；`tests/` 那十几个二进制两趟也一格没动，**命令行那一路一个字节都没碰**。

涨的四条：

| 用例 | 在哪 | 问的是 |
|---|---|---|
| `the_tail_comes_out_one_row_per_subsection_and_prints_the_same_bytes` | `render`（闸门 1、2） | 一小结一行、空着的一行都不出；`plain::tail` 拼出来的与那几行原样接下去**逐字节相同**；报告照旧以它收尾 |
| `every_tail_subsection_gets_its_own_tone_without_moving_a_glyph` | `draw::report`（只进闸门 1） | 六小结到齐、三档各就各位、每一小结头一句就是它自己那个词；屏上折出来的那几行与拼成一段折出来的**逐行相同** |
| `the_bottom_line_tells_a_refusal_apart_from_a_success` | `draw::footer`（只进闸门 1） | 三档各挂各的、三种互不相同、三个记号互不相同且都不是东亚歧义宽度、那一句连同记号真画得到屏上、**40 格上折出来的那几行拼回去那一句原样都在** |
| `the_tail_is_painted_one_subsection_at_a_time` | `draw::paint`（只进闸门 1） | 走 `TestBackend`：屏上那几行**真是什么色**——非卷文件那一小结一格没上色、隔离那一小结黄、卷级失败那一小结红 |

两条既有用例多问了一句：`the_same_screen_reads_the_same_with_or_without_colour` 多比一屏
（带屏底那一句的整屏，并按文字定位到**屏底那一行**问它是不是红的），
`the_sentences_the_table_has_no_column_for_sit_under_that_volume` 多问一句过期副本是哪一档。

**闸门之外那一遍**：`cargo fmt --check` 干净；`cargo clippy --all-targets` 与
`--all-targets --no-default-features` 两遍都零告警；`cargo doc --no-deps` 仍是
**15 条告警**（`tonefit (lib doc) generated 15 warnings`），一条没多。

## 停车场结转

**了结三条**（原文连同处置挪到这里，`## 待处理` 里已删掉，`## 已了结` 索引表各加一行），
**新记两条**（留在《待处理》里）：

| 新记的 | 一句话 |
|---|---|
| **Q197** | 隔离那一小结一句里数着两档（「隔离 N 卷 · 失败 M 页」），`Tone` 的「取最重」规矩指向出事、词条那张表指向注意——本票按词条与卷表给了「注意」 |
| **Q198** | 屏底那一句挂的是状态机自己的三档 `NoticeKind`，而语义色是四档 `Tone`：`Tone` 在 `tui` 特性后面，状态机挂不上它，两个类型说的是同一件事 |

另有**一条补在 Q163 上**（`CONTEXT.md` 词条那一类，归 28 号票）：《语义色》的「用在哪」
要添三样——过期副本进「注意」、报告末尾那几小结按小结分属三档、屏底那一句挂上三档；
那一条「每一处上色的地方旁边都另有一个字或一个行首记号」照旧成立，多出来的载体也该点一句。

### Q155 — 报告末尾那几小结整段一种色：六小结分属三档语义，而它是一段拼出来的文字

- **From:** 票 `p3-session-legibility/09`
- **Kind:** 本票认下的一处折扣
- **Where:** `src/session/draw/report.rs` 的 `collapsed`（末尾那几小结那一段），
  出处在 `src/render.rs` 的 `tail`
- **现状:** `tail` 把六小结（非卷文件 · 宽溢出 · 兜底上界 · 部分救回 · 隔离 · 卷级失败）
  拼成**一段文字**交出来，而这六件事按 spec 的《语义色》分属三档：宽溢出 · 兜底上界 ·
  部分救回是「注意」，隔离也是「注意」，卷级失败是「出事」，非卷文件连失败都不是。
  一段文字只上得了一种色，本票因此给它「平常」——**末尾那几小结一个颜色都没有**。
- **顺带落空的一档:** spec 的《语义色》把**兜底上界**列在「注意」里，而屏上眼下
  **只有末尾那一小结说得出它**（逐页那一行上的 `Field::Backstop` 要等 `p3/11` 的逐页表）。
  这一段整段给了「平常」，那一档因此在本票的地界里一处都没上色。
- **那一档已经收掉（`p3-session-legibility/11`）:** 逐页表上兜底上界改过的页
  是[要紧的页](CONTEXT.md《会话》)之一，行首挂 `!`、整行走 `Tone::Caution`、
  行尾另有「兜底上界」那个词（`src/session/draw/pages.rs` 的 `says`）。
  **本条剩下的那一半原样成立**：末尾那几小结仍是一段拼出来的文字、仍整段一种色，
  这一条要拍的板还是那个——`tail` 要不要出「一小结一段」。
- **Why it did not block:** 别的五件表上或总览块上都有着色的位置（隔离、卷级失败逐卷各占一行，
  宽溢出 · 几何门不成立 · 特例页在总览块的出事行上，部分救回在那一卷那一行底下），
  少的是「同一件事第二处也上色」；只有兜底上界是真的一处都没有，而它有一张自己的票在后面。
- **What this ticket actually did:** 整段给 `Tone::Plain`，并把**为什么**写在 `collapsed`
  那一行注释上：按小结分色要先把 `tail` 拆成一小结一段，而拆它就是把措辞挪进画法这一层
  （ADR 0016：措辞只有一处出处）。**一个字都没动 `render`。**
- **Whose call:** 拍板的人（要不要让 `tail` 出「一小结一段」，代价是措辞那一层多一个出口），
  或者收报告区那几段的下一张票
- **处置：** **了结。**`tail` 此刻出的就是**一小结一行**（`Vec<Row>`，六个新的 `RowKind`），
  会话那一头照它逐小结上色（`src/session/draw/report.rs` 的 `tail_row`）：非卷文件平常、
  宽溢出与兜底上界与部分救回与隔离注意、卷级失败出事——本条列的那三档一格不差。
  **措辞那一层没有多出一个出口**：六小结那几段一个字没动，拆的只是粒度，而
  **拼回一段的规矩挪到了纯文本那一副**（`plain::tail`）——命令行印出去的那一段
  与拆之前**逐字节相同**，`the_tail_comes_out_one_row_per_subsection_and_prints_the_same_bytes`
  钉着这一条。本条《顺带落空的一档》那一半 `p3/11` 已经收掉，
  兜底上界此刻在逐页表与末尾这一小结上都上得了色。
  会话那一头 `every_tail_subsection_gets_its_own_tone_without_moving_a_glyph` 三问：
  六小结到齐、三档各就各位、屏上折出来的那几行与拼成一段折出来的**逐行相同**。

### Q156 — 「过期副本」在四种语义里一档都不占

- **From:** 票 `p3-session-legibility/09`
- **Kind:** 规格没说
- **Where:** `src/session/draw/table.rs` 的 `under`（摆在一卷那一行底下的那两句）
- **现状:** spec 的《语义色》那张表逐条列出了四档各管哪几件事，**过期副本不在其中任何一档**。
  它与它并排的那一句「部分救回」是两种处境：后者明写在「注意」那一档里，
  前者说的是「盘上还躺着上一趟写下的那一份，这一趟没覆盖它，删不删由你」——
  既不是失败，也不是判定上要留神的事，可它也不像「正常跑完」。
- **Why it did not block:** 四档里「平常」正是「不必特别看它」那一档，摆进去不撒谎；
  而那一句本身成句、说得清清楚楚，不靠颜色。
- **What this ticket actually did:** 过期副本那一句给 **`Tone::Plain`**（部分救回给
  `Tone::Caution`，照 spec 那张表），并把这条选择写进 `under` 的文档：
  **不为它编第五档**——编出来的那一档在 `Tone` 上没有对应的语义。
- **Whose call:** 拍板的人（过期副本该不该进「注意」那一档）
- **处置：** **了结。进「注意」。**`src/session/draw/table.rs` 的 `under` 此刻两种同一档
  （`RowKind::Superseded | RowKind::Salvaged => Tone::Caution`）。理由写在那一处的文档上：
  盘上躺着的那一份是**上一趟**写下的、可能整卷都是白页，而这一趟没覆盖它也没删它——
  那正是「要留神的一件事」，既不是失败（进不了「出事」），也不是「不必特别看它」
  （那才是「平常」）。**仍旧不为它编第五档**，本条那一句原样成立。
  **颜色不是唯一载体**：那一句以「过期副本」四个字开头，措辞出自 `render`，这一层一个字没添；
  `the_sentences_the_table_has_no_column_for_sit_under_that_volume` 多问一句它是哪一档。

### Q157 — 屏底那一句没有轻重之分：跑不起来的那一句与「存下了」那一句同色

- **From:** 票 `p3-session-legibility/09`
- **Kind:** 票面没想到的第三种情形
- **Where:** `src/session/state.rs` 的 `Session::notice`（`complain` 与 `says` 那一个出口）、
  `src/session/draw/footer.rs` 的 `footer`（屏底最后那几行）
- **现状:** spec 的《语义色》那张表里「用在哪」一列没有屏底那一句。而那一格装的是两种话：
  跑不起来的那几种（型号没挑、输出根没填）、标定图写不出去，与「存下了一份预设」
  「再按一次 `d` 删掉它」摆的是同一个位置、同一个样子。
  屏上说「出事」的地方因此少一处——按 `x` 却跑不起来时，那一句与刚才那句成功的话长得一样。
- **Why it did not block:** 上色要先分得出轻重，而 `notice` 眼下是一个裸 `String`——
  给它挂一档语义要动状态机那一侧的形状（`Session::complain` 与 `says` 两个出口），
  而本票的地界是画法这一层；spec 的表里也没有它。
- **What this ticket actually did:** **一格没动**，屏底那一句照旧不上色。
  本票只上了表上那几行、总览块的抬头与出事行、失败页那一段、只读时的左栏——
  四处都在 spec 那张表的「用在哪」一列里。
- **Whose call:** 拍板的人（屏底那一句要不要分轻重），或者动屏底那一格的下一张票
- **处置：** **了结。分轻重。**`notice` 从裸 `String` 变成 `Notice`（一句话连同它是哪一种），
  **两个出口都跟着改**：写进去那一头是 `Session::says`（本模块每一句话的唯一出口，
  收的是 `Option<Notice>`），读出来那一头是 `Session::notice`（答 `Option<&Notice>`）。
  轻重那一档由状态机自己出（`NoticeKind`：没做成 / 先问一句 / 做成了）——
  `Tone` 住在画法那一层，而那一层整个在 `tui` 特性后面，状态机挂不上它（新记的 Q198）。
  画法那一头一个 `match` 把它折成语义色**连同行首那个记号**
  （`footer::marked`：`✗` 出事 / `!` 注意 / `✓` 平常），记号与语义因此绑成一对，
  与卷表那几行同一条（`table::Mark`）。`the_bottom_line_tells_a_refusal_apart_from_a_success`
  四问：三档各挂各的、三种互不相同、三个字形都不是东亚歧义宽度、那一句连同记号真画得到屏上。
