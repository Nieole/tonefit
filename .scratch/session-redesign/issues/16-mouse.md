# 16 — 鼠标：滚轮、单击与双击

**What to build:** 触控板与鼠标在会话里可用（spec《鼠标》；ADR 0019 决定第 8 条）。

- 进会话时捕获鼠标，出会话时还回去，与 raw mode 在同一处收尾，恐慌时也还；
- crossterm 的鼠标事件翻成会话输入，是一个纯函数；
- 滚轮一格把光标挪三行，挪了就暂停自动滚动；
- 画法每画一帧顺带交出**点得中的区域**：顶栏视图名、卷列表每一行、每页结果每一页、设置栏每一项、详情栏每一个值、预设栏每一行；
- 单击选中那一行（暂停自动滚动）或切视图；同一处在阈值内再点一次算双击，等于 `⏎`；阈值读会话的「此刻」。

捕获之后选屏上的字要按住 Shift 拖，这是认下的代价，不另设开关。连滚几格合成一帧在 17。

**Blocked by:** 15 — 切换到新界面

**Status:** resolved

- [x] 序列：滚轮一格挪三行；单击选中一行（自动滚动暂停）；单击顶栏视图名切视图；双击目录行展开；配置视图里单击一项、单击一个值；走完与期望屏相等
- [x] 翻译用例扩上滚轮与单击
- [x] 进出会话与恐慌时鼠标捕获都还回去，确认方式写进落地记录
- [x] 设计快照未改；三条闸门跑满

## 落地记录

- **翻译**：`terminal::translate_input` 改收整条 crossterm `Event`，一个纯函数：键只认按下那一下（从 `drive` 挪进来）、滚轮上下各一格 `Wheel(±1)`、左键按下是 `Click { x, y }`，其余放过。用例扩上滚轮、单击、抬起与 `F1`。
- **点得中的区域**：`shell::draw` 画完交出 `Vec<Hit>`（`Canvas::hit`／`hit_row` 收），六处照设计稿 `scr.hit`：顶栏视图名、卷列表停得住的行、每页结果每页、设置栏每项、详情栏停得住的格、预设栏每份。`drive` 每帧记到 `Views::hits`。`CONTEXT.md` 添词条《点得中的区域》。
- **命中与双击**（纯，特性外 `view.rs`）：`Session::click(x, y, now)` 倒着找命中段、选中；同一目标在 `DOUBLE_CLICK_WITHIN`（500ms，读会话「此刻」）内第二下回 `Clicked::Double`，终端层当一个 `⏎` 再交一次输入入口；视图名上不算双击。`Session::wheel` 走 `place_cursor` 挪 3×格数，卷列表上因此暂停自动滚动并说那一句；单击一行暂停但不说（照 `running-click-row` 那一屏，Q976）。
- **捕获收尾**：`Screen::open` 与 alternate screen 同一条 `execute!` 里 `EnableMouseCapture`；`restore()` 先 `DisableMouseCapture` 再退屏、关 raw mode，三件各收各的。
  **确认方式**：代码层——正常退出与 `?` 走 `Screen` 的 `Drop`、进到一半失败走 `open` 自己那一收、恐慌走 `hook_the_panic`，三条都只调 `restore()` 这一处。**没在真终端上看过**（agent 没有交互终端可开会话）；也没写自动用例——Windows 上 crossterm 的鼠标捕获走 WinAPI 改控制台模式、不写字节，用例里调它会改掉跑测试那个控制台的模式，写不成无副作用的断言。合并前请在真终端上进一次会话、`q` 退出后看滚轮是否还回给终端的滚动缓冲。
- **序列**：十一串鼠标序列全部经输入入口走完、逐格对设计稿（`the_wheel_the_click_and_the_double_click_walk_to_the_designed_screens`、`a_click_in_the_pages_pane_puts_the_cursor_on_that_page`）；把单击那一支故意弄坏两条都红。设计快照一个字节没动。
- **Q806**（`⏎` 在展开着的目录行上）仍没撞上：`running-dblclick-dir` 双击的是收着的目录行，照旧 ①。
- 停车场：Q976–Q979。

### 数

提交那一刻的树：闸门 1 合计 992 通过（lib 239 / bin 412，基线 981）；闸门 2 合计 877（lib 239 / bin 297，基线 870）；闸门 3 编译过；polish 全绿、doc 告警 15。
