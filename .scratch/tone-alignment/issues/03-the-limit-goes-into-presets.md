# 03: 预设里写得进

**What to build:** 常年转同一批素材的用户把上限存进自己那份预设，以后 `--preset 漫画` 一句话带上它，不必每趟重敲；而这一趟想临时改的时候，命令行上显式点名仍然覆盖得回来。

**Blocked by:** 01

**Status:** resolved

- [x] 预设的 `[preset."名字".taste]` 认 `white-align-limit`，键名就是去掉 `--` 的 flag 名，取值写法与命令行上一模一样
- [x] **命令行显式点到的那一项赢**：`--preset 漫画 --white-align-limit 0` 就是「套那一份，再改这一项」。数值参数没有那三个布尔开关的单向毛病——这正是 spec 里不另做 `--no-tone-align` 的理由
- [x] 它落在**口味层**，不在设备层：这是「这一趟愿意为对齐付多少色调」，不是面板的物理事实
- [x] 预设没说这一项时回到默认，与其余口味项同一条路
- [x] 读不懂的取值**当场报错**，不静默套默认值（与现有预设那条规矩一致）
- [x] `--preset` 在那份文件还不存在时印出来的**样例**里带上这一项
- [x] 用例照 `tests/preset.rs` 的写法
- [x] 闸门三条绿（`cargo xtask gate`）

## 落地记录

**预设的口味层认 `white-align-limit`，命令行显式点到的赢，预设没说回默认，与其余口味项同一条路。**（2026-09-11，基底 `6dbcd74`）

`TasteLayer` 加第十二项 `white_align_limit: Option<WhiteAlignLimit>` 与取值器 `white_align_limit()`
（默认值不复述，落到 `WhiteAlignLimit::default`）；`OnDiskTaste` 加 `white_align_limit: Option<u8>`——
与 `--white-align-limit` 同一个类型，256、-1、`"很宽"`、2.5 在读进来那一刻就是错误，与 clap 挡下它们是同一条界；
`resolve`／`From<&Preset>` 两处搬运各一行。命令行那一头 `Cli::white_align_limit(&self, preset)` 照 `residual_filter`
那几个的形状：点了名（含 0）就是那个数，没点用预设，预设也没说才落默认。缺文件时印的样例抽成 `const SAMPLE`
（raw string，印出来的排版逐字节同前，多 `white-align-limit = 2` 一行），并有一条用例钉着它自己就是一份读得懂的预设。

`src/session/`、`src/lib.rs`、`src/metadata.rs`、`src/cache.rs` 一个字没动。不用预设的那一趟行为不变：
`Preset::default()` 那一格是 `None`，`Request` 字段类型不变，参数哈希与黄金快照不动（`tests/golden-snapshot.txt` 在
`git status --porcelain` 下是空的）。

### 数

| | 命令 | 结果 | 对账 |
|---|---|---|---|
| 闸门 1 | `cargo test` | `945 通过 0 失败`（lib 234 / bin 393） | 基底 lib 234 / bin 390，**+3** |
| 闸门 2 | `cargo test --no-default-features` | `799 通过 0 失败`（lib 234 / bin 247） | 基底 lib 234 / bin 244，**+3** |
| 闸门 3 | `cargo check --features profiling` | 绿，末行 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.67s` | — |

三条末行都是 `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`（闸门 1、2）。
+3 是 `src/preset.rs` 那三条新用例（口味层认这一项且 0 是 `Some(OFF)`；写出去读回来、0 也写得出去；样例自己读得懂），
两条闸门同一个账。`src/main.rs` 与 `tests/preset.rs` 都是往既有用例里加格，条数不变。

`cargo xtask polish` 四条绿：`cargo fmt --check` 干净，`cargo clippy --all-targets` 与 `--no-default-features` 各 0 条，
`cargo doc --no-deps` **15 条，与基线相同**（`warning: \`tonefit\` (lib doc) generated 15 warnings`）。

### 一处按票面绕过去的地方

`preset::every_field()` 里纸白对齐上限那一格**故意留 `None`**。写 `Some` 的话，`src/session/state.rs` 的
`the_two_layers_on_screen_are_the_two_layers_a_preset_stores`（断盘上口味层键数 == `TASTE_FIELDS.len()`）当场红
——那条红是 04 号票的（口味层从十一项变十二项、屏上多一行），本票按票面不进 `src/session/`。
往返另有 `the_white_align_limit_round_trips_through_the_file` 一条钉着。代价要看见：`every_field` 那几条往返用例此刻
不盖新字段；会话里套一份写着 `white-align-limit = 2` 的预设，屏上看不见、跑按默认 4、按存原样写回。四步交接记 **Q649**。

### review 收下的

样例里那一行从 `0` 改成了 `2`：样例是用户手上唯一的格式说明，照抄一份写着 0 的会在不知情时把默认开着的那一步关掉。
进程级那条「命令行压预设」删了——`tests/preset.rs` 的模块文档明写优先级归单元测，那件事 `src/main.rs` 两条已经钉着，
进程那一层留的是 `PRESETS`／`TYPED_OUT` 各加这一项（哈希等价那条顺带盖住）与缺文件那句多断一行 `white-align-limit`。

### 停车场

**Q649–Q650** 两条：`every_field` 留白与会话侧的四步交接（给 ta/04）· `CONTEXT.md`《口味层》词条仍列十一项、与《纸白对齐上限》词条互相打架（建议 03/04 合完一次改齐）。
