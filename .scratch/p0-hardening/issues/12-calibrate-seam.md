# 12 — 标定图的落盘移进库

**What to build:** 标定图的建目录与写文件在库里完成，命令行与会话层共用同一个调用。

口径已定：**标定保持独立的第三个 seam，不并进主入口的模式。** 依据是领域模型——`CONTEXT.md` 把标定图定义为量具：不读源、不走管线、不判定、无损写出。把它塞进主入口会让请求类型的九个字段对它全无意义，结果类型也装不下一个没有卷、没有页、没有判定的产物。

要的不是「走同一个入口」，是「库里调得到、落盘在库里」。第三个 seam 满足这一条。

界面文案留在界面层，不随落盘一起搬进库。

**Blocked by:** None — can start immediately.

**Status:** resolved

- [x] 标定图的写出接口落在库内，建父目录与写文件都在库里完成
- [x] 命令行与会话层共用同一个调用，界面层不再自己碰文件系统产出产物
- [x] 界面文案仍在界面层
- [x] 标定这条路在库这一侧可测，不必起子进程
- [x] **spec 与 CONTEXT 的同步不在本票范围内**——口径已定但改文档要项目所有者点头，agent 不得顺手改

## 落地记录

第三个 seam 的形状：

```rust
pub fn write_calibration_chart(profile: &Profile, out: &Path) -> Result<()>
```

一个 profile、一个去处，回一个 `Result<()>`。画、编成 PNG、建父目录、写文件四步全在库内，
落点是 `src/calibrate.rs` 的 `write_chart`；`lib.rs` 那一层只挂公开的名字与契约文档。
它**不并进 `run`**——理由就是票面正文那一条，没有新增。

### 会话将来怎么用同一个入口

`p1-session/13` 的三条验收都落在这个签名上：

| 那张票要的 | 这个签名给的 |
|---|---|
| 「会话调库里那个写出接口，界面层不自己建目录、不自己写文件」 | 建目录与写文件都在 `write_chart` 里；调用方交出去的只有 profile 与路径 |
| 「按当前 profile 写出标定图」 | 设备层手里的就是一个 `Profile`，直接传 |
| 「写出失败…在会话里说得清，不崩掉会话」 | 失败回 `Err` 不恐慌，上下文说得出是哪件事、卡在哪个路径上 |

命令行走的是同一个调用，两边不是各写一份。C01 要重画的是图的内容，
而 seam 不碰内容——`chart_png` 恒 `BitDepth::Eight, None`，「不经判定、无损、不带记录」钉在库内。

### 命令行行为：一字未改

`fn calibrate` 这一层剩下两件命令行自己的事：把型号名与 `--gray-levels` 合成 profile，
以及印出 `render::calibration_note`。**文案没跟着落盘一起搬**——`src/render.rs` 一行未动，
`p1-session/01` 刚把它放在界面层，本票不去动它。

写出的字节、印出的文案、退出码、写不出去时的报错措辞全部与本票之前相同：
`encode::png(&chart(profile), BitDepth::Eight, None)` 这个表达式原样搬家，
两句 `with_context` 的措辞逐字保留（「建标定图的去处 …」「写标定图 …」）。

### 票外的一个决定：出字节那个公开函数撤了

原来的 `pub fn calibration_chart(&Profile) -> Result<Vec<u8>>` 降成了库内私有的
`calibrate::chart_png`。票面只说「写出接口落在库内」，没说撤掉出字节的那个接口，因此记在这里：

- **撤的理由**：`CONTEXT.md` 把第三个 seam 定义成**写出**，而全仓没有一个调用方要那串字节——
  命令行不要，`p1-session/13` 要的也是写出。留着就是一个没有调用方的公开面。
- **退回的代价**：一行 `pub`。`chart_png` 就是原来那个函数，签名一字未变。

`src/lib.rs` 顶上「对外是两个 seam」跟着改成「三个」——同一段话下面本来就列着三个，
那是 HEAD 上的旧笔误，与 CONTEXT 对齐。

### 测试

库这一侧新增两条（`src/calibrate.rs`），都不起子进程：

- `writing_the_chart_makes_its_parent_and_lays_down_the_bytes_it_drew`——
  父目录不在就建出来，写下的字节就是画出来的那张图。
- `writing_the_chart_where_the_parent_cannot_be_made_comes_back_as_an_error`——
  拿一个文件当父目录，回的是 `Err` 而不是恐慌，且说得出是哪件事、卡在哪个路径上。

命令行那一条**改写过**。原来比的是「文件里的字节 == 库现画的字节」；落盘搬进库之后
两边跑的是同一段代码，那个等号恒成立、钉不住任何东西（自检查出来的）。
改成钉这一层真正还在做的那件事——**交出去的是哪个 profile**：
写出的 PNG 头里的尺寸必须等于「Kobo Libra 2」那块面板的分辨率，
而 `--gray-levels 2` 那一趟的字节必须与不覆盖的那趟不同。

六组变异逐组验证。该红的四组都红了：删掉 `create_dir_all`（库侧与命令行侧各红一条）、
命令行丢掉 `gray_levels`、命令行把型号名换成另一台设备、两句 `with_context` 一起拿掉。
**不该红的两组没红**，那也是对的：只拿掉其中一句上下文时，另一句仍说得出是哪件事、卡在哪儿——
错误那条用例钉的是调用方看得见的结果，不是哪一步报的。

### 数

全量 **281 通过 0 失败**（基线 279）。多的 2 条都在库侧 calibrate。逐个二进制：
lib 113、bin 22、concurrency 9、container 29、exit_code 1、golden 2、idempotency 16、
isolation 15、metric 7、pipeline 57、profile 6、timing 4、smoke 0（未设 `TONEFIT_SAMPLES`，印一行跳过）。

`tests/` 一行未动，黄金基线未重录。`CONTEXT.md` 与 spec 未动（票面第五条）。
