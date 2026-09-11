//! tonefit：把漫画页适配到电子墨水阅读设备。
//!
//! 对外是三个 seam，其余全部是内部实现。
//!
//! [`run`] 是主 seam：所有模式走同一个入口，CLI 是它之上的薄层，只负责把命令行参数拼成
//! [`Request`]、把 [`Report`] 渲染成文字。
//!
//! [`score`] 是第二个 seam：判据的纯函数形态，数值与性质测试、标定工具直接调它。
//! 它周边的类型——[`Reference`]、[`Score`]、[`GrayImage`]、[`Candidate`]、[`quantize`]——
//! 一并公开，判据的调用方要拿它们拼出参照与候选。
//!
//! [`write_calibration_chart`] 是第三个：标定图。它不并进主入口——不读源、不走管线、
//! 不判定，只按一个 [`Profile`] 画出一张图并**无损写到点名的那个文件上**。
//! 量具与被处理的页走的不是同一条路。
//!
//! 三个 seam 之外另有两样对外的东西，而它们不是缝，是**两条规矩**。
//! 两条都让库担了一点界面层的事，而**理由各是各的**——不要并成一条说，
//! 各自的理由写在各自那个模块上：
//!
//! - [`glyph`]——**字形约定**那两条（[`HARD_SPACE`] 与 [`width_is_stable`]，
//!   `CONTEXT.md`《字形约定》）。同一句话从三张嘴里出来、最后一张在库内
//!   （ADR 0016 认下的那处例外），库因此会把自己造的字**直接送进一条对齐的列**里
//!   ——规矩得跟着字一起递到调用方手上；
//! - [`listing`]——**只列前几条**那个形状（[`FirstFew`]，`CONTEXT.md`《只列前几条》）。
//!   一张长清单只列前几条、剩下的报个数，而说这一句的几张嘴里**有两张在库内**：
//!   预扫那条拒绝与撞名那条拒绝，两段字都是库自己写下、直接落到 stderr 上的。
//!   收口那一句留在界面层，等于把它劈成两份。

mod cache;
mod calibrate;
mod color;
mod cost;
mod crop;
mod decide;
mod decode;
mod discover;
mod encode;
mod envelope;
mod geometry;
pub mod glyph;
mod gray;
mod interlock;
pub mod listing;
mod medium;
mod metadata;
mod metric;
mod profile;
mod progress;
mod quantize;
mod read;
mod report;
mod request;
mod resample;
mod sink;
mod source;
mod spread;
mod survey;
mod white;

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use rayon::prelude::*;

pub use cache::{CacheBudget, CacheUsage, format_bytes};
pub use color::PageColor;
pub use crop::{Crop, InkRule, ink_rule};
pub use decide::{CandidateScore, Reason, Verdict};
pub use decode::Salvage;
pub use envelope::Envelope;
pub use geometry::{FitMode, GeometryGate, Size, max_target_pixels};
pub use glyph::{HARD_SPACE, width_is_stable};
pub use gray::GrayImage;
pub use interlock::{Interlock, Voice};
pub use listing::FirstFew;
pub use medium::{ChosenBy, IoMode, IoPlan, Medium, Readers};
pub use metric::{
    Aggregation, Composition, Masking, Reference, Score, aggregation, composition, masking, score,
};
pub use profile::{Panel, Profile, Threshold, ThresholdSource};
pub use progress::{Event, Instruction, Pass, Progress, ProgressSink};
pub use quantize::{BitDepth, Candidate, Dither, quantize};
pub use report::{
    NonVolumeFile, NonVolumeReason, PageBranch, PageOutcome, PageReport, Processed, Report,
    RunOutcome, UnreachablePlace, VolumeFailure, VolumeReport, VolumeTiming, VolumeVerdict,
};
pub use request::{Mode, Request};
pub use resample::{Filter, Scaling};
pub use spread::{Cut, Gutter, ReadingOrder, Side, SplitRule, SplitThreshold};
// 认得的归档扩展名那一串：命令行的 `--help` 也要说它，而格式集只有一个出处
// （`source::ARCHIVE_FORMATS`）。见二进制侧的 `inputs_help`。
pub use source::listed_archive_extensions;
pub use white::{WhiteAlignLimit, WhiteAlignment, align_white};

use color::ColorImage;
use metadata::{
    Fingerprint, Origin, PageRecord, PageSource, PageSources, Record, Recorder, SourceHash,
};
use sink::Sink;
use source::{Member, Volume};
use spread::Split;

/// 画一张标定图并写到 `out`，父目录不在就建出来。
///
/// 尺寸恒等于面板分辨率：图要在真机上 1:1 显示才答得准。
/// 它一次上机答两件事，先后印在图内——那两件事与为什么合在一张图上，见 `calibrate` 的模块文档。
///
/// **图本身不经过位深判定**：它是量具，不是被处理的页——判据、上包络、抖动一概不碰它，
/// 像素以 8 位工作精度原样交给编码器，写出的是无损 PNG。自描述元数据也不写：
/// 记录说的是一页的判定与幂等依据（见 `metadata`），标定图两样都没有。
///
/// 落盘在库内完成，命令行与会话共用这一个调用（加固批 12 号票）：出图这件事从头到尾只有一份，
/// 界面层两边都不必自己建目录、自己写文件。写不出去时回的是 `Err`——盘满、
/// 父目录建不了都在里面，调用方接住它照自己的方式说，不必崩掉一整个会话。
///
/// 印在终端上的那几行不在这里：那是**界面文案**，随调用方走（见二进制侧的 `render`）。
pub fn write_calibration_chart(profile: &Profile, out: &Path) -> Result<()> {
    calibrate::write_chart(profile, out)
}

/// 在点名的若干路径底下**发现**卷，逐卷处理，产出设备优化副本。源库只读。
///
/// 点名的是**在哪里找**，不是**找到什么**：一个路径展开成它底下的那一批卷
/// （ADR 0014，见 `crate::discover`），输出按源的结构镜像到输出根下。
///
/// # 两种失败分得开（05 号票）
///
/// **拒绝执行**回的是 `Err`：错在这一趟的**参数**上，换一个卷不会变好，整趟因此当场停
/// （见库内的 `Refusal` 与 `crate::survey`）。**哪几种算拒绝执行**，单子在
/// `CONTEXT.md` 的《失败》——那里连同「哪几种发生在开工之前」一起写着，这里不抄第二份。
///
/// 开工之前那几种一页都不做；**几何门那一种不是**——几何门是页的事实，
/// 要真撞上那一页才拦得住（见 `Candidates::for_gate`），那时先做完的卷已经在盘上，
/// 而调用方拿到的是错误、没有报告。那是这条路唯一说不出「一页都没做」的地方。
///
/// **卷级失败**回的是 `Ok`：预扫时打得开、轮到它却做不成的卷（**卷根整个不见了**、
/// 文件被删、盘拔了、权限变了、透传文件搬不动）记进 [`Report::failed_volumes`]，
/// 其余卷照做、报告照出。
/// 「一卷点不开就毁掉整趟」正是这条分岔要改掉的毛病——那时前面几十卷的输出还在盘上，
/// 而那份说得清它们是什么的报告全丢了。
///
/// **两样都不是**：发现出来的归档点不开、一页都没有的东西——它们连卷都不是，
/// 进的是 [`Report::non_volume_files`] 那张并列的第三张表，逐条带着路径与一句为什么，
/// 退出码一格不动（ADR 0014 决定第 3、5 条）。
///
/// # 走不进去的地方（`p4-parking-lot/11`）
///
/// 发现的时候**列不出某一层**（权限没配好、盘掉了）时那棵子树整个跳过，其余卷照做——
/// 而这一趟记得住是哪一个地方、为什么（[`Report::unreachable_places`]）。
/// 它不进上面那张表：那张列的是文件，而这是一个地方（见 [`UnreachablePlace`]）。
/// **它动退出码**：一棵没看过的子树不是「全都做成了」。
pub fn run(request: &Request) -> Result<Report> {
    // 整趟的表从这里开始掐：开工前那几道检查也要摸文件系统，摊在计时之外
    // 只会让报出来的总耗时比调用方自己在外面掐的那个小一截（加固批 11 号票）。
    let started = Instant::now();
    // 分阶段耗时剖面的表在这里清零：它说的是**这一趟**（见 `cost::start`）。
    // 特性关着时这一句什么都不做。
    cost::start();
    if request.inputs.is_empty() {
        bail!("处理范围为空：至少点名一个在里面找卷的地方（ADR 0009：处理点名的子集）");
    }
    ensure_the_overrides_leave_a_candidate(request)?;
    for input in &request.inputs {
        ensure_output_is_elsewhere(input, &request.output_root)?;
    }
    // 排在预扫之前：它是开工前这几道里唯一往盘上写东西的，而预扫要走一遍点名的那几棵树、
    // 把发现出来的每一个卷都枚举一遍，输出根根本没地方落时那一趟是白付的（06 号票，见
    // [`ensure_the_output_root_takes_a_write`]）。撞名那一道排在预扫**之后**——
    // 撞在一起的是发现出来的那些卷，发现之前问不出来（见
    // [`ensure_no_two_volumes_share_an_output`]）。
    ensure_the_output_root_takes_a_write(&request.output_root)?;
    // 介质**按路径**探测，一次运行共用一份缓存（ADR 0009 决定第 2 条，见 `medium`）：
    // 同一趟里源卷可能在仓库盘上、输出在系统盘上，逐卷各判各的，互不影响。
    let mut probes = medium::Probes::new();
    // 这一趟的事件流。闩活在这里——一次运行一份，`Request` 复用不到它
    // （见 [`progress::Events`] 的 `standing`）。在决策点上等人等掉的那一截同一条寿命。
    let standing = progress::Standing::default();
    let deliberation = progress::Deliberation::default();
    let events = progress::Events::new(request.progress.as_ref(), &standing, &deliberation);
    // **预扫**：开工之前发现这一趟有哪些卷，把它们全枚举一遍，算出这一趟的全局总步数
    // （ADR 0011 决定第 3 条、ADR 0014）。它排在开工那条事件**之前**，因为那条事件要带着
    // 那个数；点名的坏路径因此在任何卷级事件之前就把整趟拒掉——输出根下一个文件都没有
    // （见 `survey`）。
    let survey = survey::Survey::of(request)?;
    // 撞名要在写出第一个字节之前说，而**撞在一起的是发现出来的那些卷**——点名的是
    // 「在哪里找」，不是「找到什么」（ADR 0009 决定第 1 条）。这一道因此排在预扫之后、
    // 开工那条事件之前。
    ensure_no_two_volumes_share_an_output(survey.volumes(), &request.output_root)?;
    // 开工前那几道检查与预扫都排在它之前：那几种失败一条事件都不发，调用方拿到的是错误本身。
    // 报的是**发现出来的卷数**，不是点名了几个路径：进度条上那个分母得是真要做的那些。
    events.run_started(survey.volumes().len(), survey.steps());
    let mut volumes = Vec::with_capacity(survey.volumes().len());
    let mut failed_volumes = Vec::new();
    let mut outcome = RunOutcome::Completed;
    // 预扫的**三份产出**一起交出来（见 `survey::Survey` 的那个同名方法）：卷这一份在下面
    // 被逐个吃掉，另两份原样挂到报告上。它们整份在开工之前就齐了——发现走完
    // 就不再变，因此按停停在半路的那一趟，这两张表照样是全的。
    let (surveyed_volumes, non_volume_files, unreachable_places) =
        survey.into_volumes_and_the_rest();
    for surveyed in surveyed_volumes {
        // **卷边界上的检查点**（ADR 0013 决定第 1 条）：收尾让当前卷跑完就停，
        // 而「当前卷跑完」正是这里——盘上因此只有完整的卷，下一趟幂等接着走。
        // 中止在这一道上与收尾同样停下：力度更强的指令不该比更弱的那个停得更晚。
        if events.standing() != Instruction::Continue {
            outcome = RunOutcome::of(events.standing());
            break;
        }
        // 卷根在这里先留一份：`process_volume` 要把这一格吃进去，而没做成的那一卷
        // 仍然得指得出自己是谁。一卷一次克隆，摊不到页上。
        let root = surveyed.root.clone();
        match process_volume(surveyed, request, &mut probes, events) {
            Ok(Some(report)) => volumes.push(report),
            Ok(None) => {
                // **中止**（ADR 0013 决定第 2 条）：这一卷停在页边界上、那格 `partial` 已经丢掉，
                // 它等于没做，报告里因此没有它这一条。下一卷更不必开工——卷边界那个检查点
                // 也会拦下它，这里明写是为了让「中止掉的卷不进报告」与「后面的卷不做」
                // 在同一处看得见。
                outcome = RunOutcome::of(events.standing());
                break;
            }
            // **拒绝执行**：错在这一趟的参数上，换一个卷不会变好（见 [`Refusal`]）。
            // 整趟当场停，返回的是那个错误本身——退出码 `1`，不是卷级失败那个 `3`。
            // 收场那一条照发：开工报过了，收场就得报得到（见 `Event::RunFinished`）。
            Err(error) if error.downcast_ref::<Refusal>().is_some() => {
                events.run_finished(RunOutcome::Refused);
                return Err(error);
            }
            // **卷级失败**（05 号票）：预扫时打得开、轮到它却做不成的卷记一笔，
            // 其余卷照做、报告照出。整趟当场失败的话，前面几十卷的报告跟着一起没了，
            // 而它们的输出还好好地躺在盘上——那是几十卷的长任务里最难受的一种结局。
            Err(error) => {
                let reason = format!("{error:#}");
                events.volume_failed(&root, &reason);
                failed_volumes.push(VolumeFailure {
                    volume: root,
                    reason,
                });
            }
        }
    }
    events.run_finished(outcome);
    // 分阶段耗时剖面（加固批 13 号票的量具）。特性关着时这一句什么都不做，见 `cost`。
    cost::print_profile();
    Ok(Report {
        profile: request.profile.clone(),
        fit: request.fit,
        crop: request.crop,
        split: request.split,
        white_align_limit: request.white_align_limit,
        volumes,
        failed_volumes,
        non_volume_files,
        unreachable_places,
        outcome,
        // 在决策点上等人的那几分钟不算这一趟的账（停车场 Q41）：库那时一步都没走。
        // 各卷的 `VolumeTiming::elapsed` 各自减掉自己那一截，这里减的是全部卷的和。
        elapsed: started.elapsed().saturating_sub(events.deliberated()),
    })
}

/// **拒绝执行**：错在这一趟的参数上，不在这一卷上（`CONTEXT.md` 的《失败》）。
///
/// 卷级失败与拒绝执行在 [`process_volume`] 的返回值上长得一样——都是 `Err`——
/// 而两者的处置正相反：前者记一笔、其余卷照做（退出码 `3`），后者整趟当场停
/// （退出码 `1`）。分辨它们的只有这个标记，`run` 靠 `downcast_ref` 认它。
///
/// **眼下只有一处**戴它：覆盖项把候选集裁空（见 [`candidates`] 与 [`why_nothing_is_left`]）。
/// 其中互锁 ③ 那一支的处置明写着「维持拒绝」（页几何批 05 号票），而它撞得上的时机
/// 在第一遍里、一页一页地判（见 [`Candidates::for_gate`])——真落到卷级失败那条路上，
/// 「拒绝」就悄悄降级成了「这一卷没做成」，而用户点的 `--dither fs` 对每一卷都错。
///
/// 装的是那句话本身而不是包一层 `anyhow::Error`：那一句要在**每一张**撞上门的页上
/// 各说一遍，而 `anyhow::Error` 复制不了（见 [`Candidates::for_gate`]）。
#[derive(Debug)]
struct Refusal(String);

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refusal {}

/// 掐一段的表：跑一遍 `work`，把这一段的墙钟耗时写进 `segment`。
///
/// 写成一个函数而不是在调用处各写三行，为的是让「哪几段掐了表」在 [`process_volume`] 里
/// 一眼数得清：段与段不许重叠，而重叠一旦发生，[`VolumeTiming`] 里三段之和就会大于总耗时。
fn timed<T>(segment: &mut Duration, work: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let value = work();
    *segment = started.elapsed();
    value
}

/// 这一卷要走多少步（spec 的 story 30）。
///
/// **预扫**算它，一卷一次（见 `survey`）：开卷那条事件报的是它，这一趟的全局总步数是
/// 它们的和。两个数因此不会分家——不是各算一遍，是加出来的。
///
/// 四段：**摊开**这一道落全部成员，幂等这一道读全部**源**成员，第一遍走每一张**源页**，
/// 第二遍写全部**输出**成员。
/// 源那一侧与输出那一侧不是同一个数——一个源页产出一到多张输出页（页几何批 03 号票），
/// 而几张由内容决定（有没有装订沟，页几何批 04 号票）。四段里只有末一段按输出那一侧算：
/// 读源与解源页都发生在切开之前。
///
/// 各段自己可能不在——**摊开那一段只有固实归档有**（`.7z` / `.rar`，ADR 0015 决定第 3 条；
/// 判据见 `source::Volume::extracts_before_work`），`--no-metadata` 关掉幂等那一段
/// （那时既没有记录可写也没有依据可比），dry-run 没有末一段（一个文件都不落盘）。
/// 因此按**这一趟真要做的事**算，而不是按一个固定的倍数：
/// 不然进度条会停在某个百分比上再也不动。
///
/// 摊开那一段是 `p4-parking-lot/13` 添的：那一段从前一步都不报，几百兆的卷在那里
/// 进度条一动不动。添进来的同时预告也跟着长，**「预告是上界」因此一格没动**——
/// 只报步不改预告的话，固实归档上进度条会冲过头。
///
/// 幂等命中的卷会提前收摊，那时走过的只有第一段——预告的步数是**上界**，不是承诺，
/// 剩下的由 [`Event::VolumeFinished`] 一次性了结。**按页跳过的卷同理**（two-pass-rework/14）：
/// 留下的页第一遍不走，那几步少报；第二遍它们照样一步一张（搬也是写）。哪几页会留下
/// 要幂等那一道比过才知道，而预扫在它之前，预告因此减不掉它们。
///
/// 第二段那个数**也是上界**，理由与上面那条不同：一个源页产出几张要解了像素才知道，
/// 而这一步在解码之前。取的是[每个源页最多几张](MAX_OUTPUTS_PER_SOURCE_PAGE)——
/// 一卷里真被切开的页越少，走过的步就越少。取下界会让进度条冲过头，
/// 而「预告是上界」这条规矩本来就在。
fn volume_steps(members: MemberCounts, request: &Request) -> u64 {
    let MemberCounts {
        source_pages,
        output_pages,
        extras,
        extracted_members,
    } = members;
    let fingerprint = if request.metadata {
        source_pages + extras
    } else {
        0
    };
    let write = if request.mode == Mode::Process {
        output_pages + extras
    } else {
        0
    };
    (extracted_members + fingerprint + source_pages + write) as u64
}

/// 一个卷这一趟要碰的成员数，摊开那一侧、源那一侧与输出那一侧分开数（页几何批 03 号票）。
///
/// 数**两遍**（见 [`MemberCounts::of`]）：预扫数一遍算出步数，处理那一卷时按重开的那一份
/// 再数一遍。报告里的数出自后一遍——它才是真做了的那一卷。
///
/// 几个数绑成一个类型而不是几个相邻的 `usize` 参数：它们总是一同算出、一同传下去，
/// 而几个同型的裸数换了位置编译器一句话都不会说，[`volume_steps`] 却会当场少报或多报一整段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MemberCounts {
    /// 源页数。幂等这一道读它们，第一遍走它们。
    source_pages: usize,
    /// 输出页数的**上界**。第二遍写它们——一个源页产出一到多张，切开发生在第一遍之内，
    /// 而几张由内容决定（页几何批 04 号票），因此这一步只给得出上界。
    output_pages: usize,
    /// 透传文件数。它不经切开，两侧数的是同一批。
    extras: usize,
    /// **开工前摊开**要落的成员数：固实归档是整卷（源页加透传），不摊开的卷是 0
    /// （`p4-parking-lot/13`）。
    ///
    /// 它与源那一侧数的是同一批成员，却单占一格：**那一段自己会不会走由容器与格式定**
    /// （见 `source::Volume::extracts_before_work`），而另外那几段在不在由这一趟的模式定
    /// （第一遍恒走，幂等那一道看 `--no-metadata`，第二遍看 dry-run）。
    /// 拿 `source_pages + extras` 在 [`volume_steps`] 里现算的话，
    /// 「这一卷摊不摊开」就得再传一个真假进去，而那正是这个类型存在的理由。
    extracted_members: usize,
}

impl MemberCounts {
    /// 数一个打开了的卷。
    ///
    /// 两个调用点各数各的那一遍：预扫按它算这一卷的步数（见 `survey`），
    /// [`process_volume`] 按**重开的那一份**再数一次。公式因此只有一处——
    /// 两边各写一份的话，「预告了多少步」与「报告说做了多少页」会各自漂。
    fn of(volume: &Volume, request: &Request) -> Self {
        let source_pages = volume.pages.len();
        let extras = volume.extras.len();
        Self {
            source_pages,
            // 上界，不是承诺：一卷里真被切开的页越少，第二遍走过的步越少
            // （见 [`volume_steps`]）。
            output_pages: source_pages * max_outputs_per_source_page(request),
            extras,
            // **只问容器与格式，不看这一卷此刻摊开了没有**：预扫数的那一遍卷还没摊开
            // （`source::enumerate` 交出来的读取端取不出字节），而两遍要数出同一个数。
            extracted_members: if volume.extracts_before_work() {
                source_pages + extras
            } else {
                0
            },
        }
    }
}

/// 锁上这一卷的缓存。
///
/// 中毒了照样用：里面是这一卷的账本，而一条计算线程恐慌不该让其余每一条跟着恐慌——
/// 那会把一处失败放大成整趟失败，真正的恐慌还被「锁中毒了」这句话盖住。
/// 与读取层那道闸同一条规矩（见 `read` 的 `Throttle::lock`）。
///
/// 交出来的不是一把裸 `MutexGuard` 而是 [`CacheGuard`]：调试构建上它多带一枚
/// [哨兵](progress::LockSentinel)，好让「持着它去报到」当场炸掉。
fn lock(cache: &Mutex<cache::PageCache>) -> CacheGuard<'_> {
    CacheGuard {
        held: cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        #[cfg(debug_assertions)]
        _sentinel: progress::LockSentinel::new(),
    }
}

/// 持着这一卷缓存那把锁的凭据：一把 `MutexGuard`，外加调试构建上的那枚
/// [哨兵](progress::LockSentinel)。
///
/// 用起来与 `MutexGuard` 无异（`Deref`/`DerefMut`），[`lock`] 的调用处一个字都不必改。
/// 发布构建上那一格不在（见 [`progress::LockSentinel`]），它退化成 `MutexGuard` 本身。
struct CacheGuard<'a> {
    held: MutexGuard<'a, cache::PageCache>,
    /// 只为存在与析构，没人读它。发布构建上这一格不在。
    #[cfg(debug_assertions)]
    _sentinel: progress::LockSentinel,
}

impl Deref for CacheGuard<'_> {
    type Target = cache::PageCache;

    fn deref(&self) -> &Self::Target {
        &self.held
    }
}

impl DerefMut for CacheGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.held
    }
}

/// 计算层这一趟摊得开多少条。
///
/// 取核数：rayon 的默认线程池就是照这个数建的，读取层无谓比它派得更多——读得再快，
/// 也没有更多的核去消化（见 `read`）。
fn cores() -> usize {
    num_cpus::get().max(1)
}

/// 隔离目录在输出根下的名字（12 号票：含失败页的卷输出到隔离目录）。
///
/// 名字用 ASCII：输出常常要经 MTP 或 FAT 搬到阅读器上，目录名少一分编码上的赌注是一分。
/// 下划线前缀买两件事——它不至于撞上一个真叫这个名字的卷，列目录时也排在最前面。
const ISOLATED_DIRECTORY: &str = "_isolated";

/// 处理一个卷：第一遍解码到判据，第二遍量化到写出，非图片成员原样搬过去。
///
/// 两遍之间隔着缓存（ADR 0005：解码一次，缓存缩放后的图）。第二遍的输入是第一遍存下的参照，
/// 源页因此只被解码一次——`VolumeReport::decodes` 是这条不变量看得见的形式。
///
/// 彩页在彩色 profile 下不走这条路：它在第一遍就缩放并编好，绕开缓存、判据与汇总
/// （ADR 0005 决定第 4 条，见 [`first_pass`]）。第二遍只把它按阅读顺序写出去。
///
/// dry-run 走同一条路，只是不建输出容器，第二遍也就没有可写的地方。
///
/// 两遍之前还有一道**幂等**：上一趟的输出还在、依据一项没变，这一卷就整个不做
/// （见 [`volume_fingerprint`] 与 [`compare_with_the_prior_output`]）。dry-run 也走这一道——
/// 它预告的是照做时会发生的事，而照做时会发生的正是「跳过」（spec 的 story 6、story 8）。
///
/// **依据按这一趟的作用域只有一种**（two-pass-rework/15；`CONTEXT.md` 的《源哈希》）：
/// 一页的字节由全卷定（`--envelope` 那条路）就按卷——全卷一个源哈希，整卷跳或整卷重做；
/// 只取决于它自己（默认路径）就按页——每一页自己一份，**卷不齐时按页**（two-pass-rework/14；
/// ADR 0018 决定第 4 条）：页级依据对得上的页**留下**——不读、不解、不判、不编，第二遍从
/// 上一趟的输出里原样搬过来；对不上的页重做。卷仍是去处、撞名、透传文件的单位：留下的与
/// 重做的按阅读顺序一起写进同一个容器，收尾照旧整个换掉（见 `crate::sink`）。
///
/// **卷的去处到第一遍走完才定得下来**（12 号票）：有失败页的卷整个进隔离目录，
/// 而哪一页失败要解过才知道。输出容器因此在第一遍之后才建——写出全在第二遍，
/// 早建一步只会让隔离的卷在干净的去处留下一个空壳。
///
/// **隔离的卷不被幂等跳过**：跳过只认干净的那个去处（见 [`compare_with_the_prior_output`]）。
/// 这是有意的——那不是一份做完了的输出，而失败清单每一趟都要重新给得出来（spec 的 story 26）。
/// 代价是有坏页的卷每趟都重做一遍，直到坏页被修好——按页那一支上重做的只是与干净去处里
/// 那一份对不上的页，坏页修好之后没变的页照样留下。
///
/// `probes` 是这一趟共用的那份介质探测（ADR 0009 决定第 2 条，见 `medium`）。这一卷问它
/// 一次，答案变成一份[读取计划](IoPlan)：这一卷读几条、为什么是这个数，报告照它说。
/// **问在重开这一卷之后**——探的是这一卷此刻真正住的那个路径，而摊开的卷要摊开了
/// 才住得进临时目录里去（见本函数里那一句上的注释）。
///
/// 收的是一份**预扫摘要**（见 `survey`）——这一卷的路径与几个数，不是卷本身：
/// 预扫数完就把卷放掉了，这里**按那个路径再开一次**。为什么宁可读两遍中央目录也不攥着它，
/// 见 `survey` 的模块文档。重开这一遍落在这一卷的墙钟之内，成员也按重开的这一份重新数
/// （见 [`MemberCounts::of`]）。
///
/// # 中止：回 `None`
///
/// **[页边界那个检查点](progress::Events::aborting)问在这几处**（ADR 0013 决定第 2 条）。
/// 凡是**逐个成员**往下走的循环，循环头上都问一次——开工前[摊开一整卷](source::open)
/// 那两遍顺序扫（`source::spread_seven_zip` 与 `spread_rar`，`p4-parking-lot/13`）、
/// 幂等这一道、第一遍、第二遍写页、第二遍搬透传文件；**外加一处不是循环头的**：
/// 本函数里 `source::open` 紧接着那一句——摊开途中按下的那一下要在那里收口，
/// 它交出来的是一份半摊开的卷（见那一句上的注释）。
/// 答中止就当场停下，这一卷回的是 `None`。
/// 不逐个数它们，也不在别处复述这个清单：数目会随管线长，而这里是它唯一的出处。
/// 前两处**落在读取那一层**，不在本函数里——「唯一的出处」说的是这张清单，不是这个文件。
///
/// `None` 说的是**那一卷等于没做**：它那格 `partial` 没有收尾、由析构丢掉
/// （见 `crate::sink` 的两个 `Drop`），最终位置上一个字节都没动过，报告里因此
/// 也不该有它的位置。第二遍开始之前中止的话连那一格都还没建。
///
/// 每一段停下之后都**再问一次**闩，而不是把「我是被中止的」当成返回值一层层传上来：
/// 闩只升不降，再问一次恒得同一个答案（见 [`progress::Events::aborting`]）。
///
/// # 收尾：停在决策点上
///
/// **续做的决策点**在「汇总之后、第二遍之前」，一卷一次（ADR 0012 决定第 2 条）：
/// 答继续就往下做，答收尾就**停在这儿**。停下来的现场与中止不同，两件事都要看清——
///
/// - 回的是 `Ok(Some(report))`，不是 `None`：这一卷**做过事**，判定、逐页结果、缓存用量、
///   解码计数都是真的，只是第二遍一步没走。那正是 dry-run 的效果（spec 的 story 6），
///   而报告本来就是试算要看的那份东西。
/// - **输出一个字节都不写**：输出容器连建都不建，`partial` 因此也没有。
/// - **参照还在缓存里**：这一卷的缓存活到 `run` 走完（ADR 0012 决定第 4 条），
///   会话答继续的那一次由同一趟 `run` 接着做——续做不跨调用。
/// - 这个字照样进闩，所以**剩下的卷不必开工**：卷边界那个检查点接着拦下它们。
///
/// 这一处认的是**当场答的那个字**，不是闩——为什么，见
/// [`progress::Events::ask_before_the_second_pass`]。
///
/// # 失败：回 `Err`
///
/// 这一卷做不成就回 `Err`，**整趟不因此停下**（05 号票）：`run` 把它记成一笔
/// [卷级失败](VolumeFailure)，其余卷照做（见 `run` 的《两种失败分得开》）。
/// 卷根整个不见了（见 [`ensure_the_volume_root_is_still_there`]）、重开这一卷点不开、
/// 撞名、指纹那一道读不出字节、第一遍读不出源、建不出输出容器、透传文件搬不动，
/// 都从这条路出去。
///
/// **一个例外**：戴着 [`Refusal`] 的那种错误说的是「这一趟的参数错了」，
/// `run` 认出它就整趟当场停。这里不必分辨两者——标记在造错误的地方戴上，
/// 这一层只管把错误交出去。
fn process_volume(
    surveyed: survey::Surveyed,
    request: &Request,
    probes: &mut medium::Probes,
    events: progress::Events,
) -> Result<Option<VolumeReport>> {
    // 这一卷的两个可能去处。哪一个作数要等第一遍走完才知道，另一个则可能留着上一趟的过期副本。
    //
    // 两个都由预扫交过来的那条**镜像相对路径**接出来（见 `survey::Surveyed::output_path`）：
    // 输出镜像源的结构，基准点是点名路径的父目录（ADR 0014 决定第 4 条）。
    // 隔离目录只是在中间插一级 `_isolated`，镜像出来的结构一模一样。
    let clean = surveyed.output_path(&request.output_root);
    let isolated = surveyed.output_path(&request.output_root.join(ISOLATED_DIRECTORY));
    // 借住在这一卷去处里的那些卷：收尾换掉的范围按它收窄（`sink::DirectorySink`）。
    // 它相对的是**这一卷的去处**，干净去处与隔离目录里那个去处因此共用同一份
    // （见 `sink::Lodgers`）。
    let survey::Surveyed {
        root,
        steps,
        enumerating,
        lodgers,
        ..
    } = surveyed;
    // 这一卷的表：三段各自掐（加固批 11 号票，见 [`VolumeTiming`]）。总的那个数从这里起算，
    // 也就是**在重开这一卷之前**；**再把预扫枚举它的那一截加回去**——枚举两遍都是这一卷
    // 真花掉的时间，一遍在这个表里，一遍由预扫交过来（见 `survey::Surveyed::enumerating`），
    // 而 `outside_the_segments` 的文档正指着它说「少掉的那一截恰恰是枚举」。
    let started = Instant::now();
    // **开卷时**的累计读数，与 `started` 成一对。这一卷的墙钟要减掉「在决策点上等人」
    // 的那一截（停车场 Q41），而那一截就是这个快照与拼报告时那个读数之差——
    // 累计只升不降，见 `progress::Deliberation`。
    let deliberated_at_open = events.deliberated();
    // 这一卷的墙钟：从重开这一卷（外加预扫枚举它的那一截）算到这份报告成型，减去等人的那一截。
    let wall_clock = || {
        let deliberated = events.deliberated().saturating_sub(deliberated_at_open);
        enumerating + started.elapsed().saturating_sub(deliberated)
    };
    let mut timing = VolumeTiming::default();
    // 开卷那一条排在**这一卷的第一件事之前**：往后每一条出口——一卷跑完、卷级失败、
    // 中止——都在它之后，画进度的那一层因此不必分「这一卷开过头没有」两种情形
    // （见 `progress::Event::VolumeFailed`）。它排在下面那道「卷根还在不在」之前
    // 正是为了这个：那一道是这一卷最早的一个 `Err`。
    // 它报得出卷根与步数，靠的正是预扫留下的那两样——重开还没发生，这里也不需要它发生。
    events.volume_started(&root, steps);
    // 卷根在预扫之后整个消失是**这一卷没做成**，不是「这一卷全是坏页」（05 号票）。
    // 判据、为什么非得在这里单问一句，见 [`ensure_the_volume_root_is_still_there`]。
    ensure_the_volume_root_is_still_there(&root)?;
    // **按路径再开一次**：预扫只数不留，卷在它手上已经放掉了（见 `survey`）。
    // 开在这里而不是更早，是因为上面那两句一个要在最前、一个要抢在真读字节之前。
    //
    // **固实归档就在这一句里摊到临时目录**（ADR 0015 决定第 3 条）——「开工前」指的正是
    // 这个位置：卷根还在的那一道已经过了，而下面每一件要源字节的事都还没开始。
    // 摊不下（磁盘不够）从这里回 `Err`，那是卷级失败，其余卷照做。
    //
    // **观察者那条回路一并递进去**（`p4-parking-lot/13`）：摊开一整卷要跑很久，
    // 那一段里报得出步、也停得住（见 `source::open`）。
    let mut volume = source::open(&root, events)?;
    // **页边界那个检查点**，摊开途中按下的那一下在这里收口：`source::open` 交出来的
    // 是一份**半摊开**的卷（成员表齐、临时目录里只有停之前落下的那几个），
    // 底下每一件事都要源字节，一件都不能做。丢掉它连临时目录一起收走（见 `source::Extraction`），
    // 第二遍还没开始、一格 `partial` 都还没建，最终位置纹丝不动。
    if events.aborting() {
        return Ok(None);
    }
    // 摊了多少字节先留一份：报告两处都要它，而其中一处（跳过那一支）会把卷根搬走。
    let extracted = volume.extracted();
    // 成员按**重开的这一份**数，不是预扫那一份：报告说的得是真做了的这一卷
    // （见 [`MemberCounts::of`]）。
    let members = MemberCounts::of(&volume, request);
    // 这一卷的输出成员名此刻只预告得出**一对一那一套**：一个源页产出几张要解了像素才知道
    // （有没有装订沟，页几何批 04 号票），而这一步在解码之前。撞名因此查两遍——
    // 这一遍拦下与内容无关的那些（`001.jpg` 与 `001.png` 撞在同一个输出上、归档里的同名成员），
    // 真正产出的那批名字等第一遍走完再查一遍。早查这一遍买的是**别白做一整卷**。
    ensure_one_member_per_output(&volume, &one_to_one_targets(&volume))?;
    let source_pages = members.source_pages;

    // **介质按这一卷此刻真正住的那个路径探**（ADR 0009 决定第 2 条）：按路径探测那条边界
    // 一格没动，换的只是探哪一个路径。摊开的卷的字节此刻一个成员一个文件地躺在一个临时目录
    // 里，那条读取通道与这个归档来自哪块盘不再是同一条（见 `source::Reader::reads_from`）。
    // **因此非探在这里不可**：那个临时目录要 `source::open` 摊开了才存在。
    //
    // 派几条读取同样按[读取端](source::ReadingEnd)分，不按容器形态——摊开的卷
    // 「之后完全按目录卷走」（ADR 0015 决定第 3 条），它的 `container` 却仍是归档。
    let medium = probes.medium(volume.reader.reads_from());
    let io = IoPlan::decide(
        medium,
        request.io_mode,
        volume.reader.reading_end(),
        cores(),
    );
    let writes = request.mode == Mode::Process;
    // 两套候选在碰卷之前备好，页判出门之后现取一套（见 [`Candidates`]）；
    // 这一卷的档什么时候定得下来，决定灰度页那一格缓存里装的是参照还是编好的字节
    // （见 [`Settles`]）。两样从前在第一遍里备，提到这里是因为幂等那一道也要问后者：
    // **页级依据在这一趟成不成立**，答的是照做那一趟的形态（见 [`Settles::if_processing`]）。
    let candidates = Candidates::new(request)?;
    let settles = Settles::for_this_run(request, &candidates);
    // 上一趟写在干净去处的输出，比对与留下的页都从它来（two-pass-rework/14）。
    let prior_output = request
        .metadata
        .then(|| sink::Written::open(&clean, volume.container))
        .flatten();
    // 源哈希按哪种作用域算、记、比（two-pass-rework/15）：这一页的字节只取决于它自己（第一遍
    // 就编好）就按页，由全卷定就按卷——一处出处，与 13 号票写不写页级依据的是同一句
    // （`CONTEXT.md` 的《源哈希》）。问的是**照做那一趟**的形态，试算要预告的是照做时会发生的事。
    let walks = Settles::if_processing(request, &candidates);
    let by_page = walks.encodes_in_the_first_pass();

    // `--no-metadata` 关掉记录，幂等的依据无处可写也无处可读，这一整道于是不在。
    //
    // 算指纹与拿它比是**同一段**（`CONTEXT.md` 的《管线》：算出本卷的指纹，与上一趟写在
    // 输出里的比）：比的那一半要开输出容器、逐成员读回记录，同样是真 I/O。摊到段外，
    // 「跳过一卷花在幂等上多久」就会少算一截，而那正是这个数存在的理由（加固批 11 号票）。
    // 比出来的答案有三种（见 [`Reuse`]）：整卷跳、按页留、整卷重做。
    let mut reuse = Reuse::Nothing;
    let fingerprint = if request.metadata {
        events.pass_started(Pass::Fingerprint);
        timed(&mut timing.fingerprint, || -> Result<_> {
            let fingerprint = volume_fingerprint(&mut volume, request, &io, by_page, events)?;
            // 中止之后**不再问幂等**。不是因为答案会错——下一句就把整卷连同这个答案一起
            // 丢掉了——而是因为问一次要开上一趟的输出容器、逐页读回记录，那是实打实的 I/O。
            // 「立刻停」停的正是这种活。手上那份哈希此刻也只喂了一半，它同样走不出这一卷。
            if !events.aborting() {
                reuse =
                    compare_with_the_prior_output(prior_output, &volume, &fingerprint, &lodgers);
            }
            Ok(Some(fingerprint))
        })?
    } else {
        // 掐表在这个 `if` 之内：整道不在时那一段是零，而不是一个「什么都没做」的很小的数。
        None
    };
    if events.aborting() {
        // 中止停在幂等这一道上：第二遍还没开始，一格 `partial` 都还没建，
        // 最终位置纹丝不动（见本函数的《中止：回 `None`》）。
        return Ok(None);
    }
    // 三种答案三条路（见 [`Reuse`]）：整卷跳过在这里就收摊；按页那一支带着留下的页与
    // 打开着的上一趟输出往下走（two-pass-rework/14）；整卷重做一页都不留。
    // 往下第一遍只走要重做的那些源页，第二遍按阅读顺序把留下的照搬、重做的写出。
    let (retained, mut prior_output) = match reuse {
        Reuse::Whole { page_count } => {
            let report = VolumeReport {
                volume: volume.root,
                output: clean,
                // 跳过的卷是干净的，隔离目录里若还留着一份，那是上一趟坏页时写的。
                superseded: superseded(&isolated),
                pages: Vec::new(),
                retained_pages: 0,
                source_pages,
                verdict: Some(VolumeVerdict::Skipped { page_count }),
                cache: CacheUsage::new(request.cache_budget),
                // 跳过的卷这一格**不是零**：幂等那一道照样把整卷的字节读了一遍，
                // 而读之前得先摊开（见 `VolumeReport::extracted`）。
                extracted,
                decodes: 0,
                // 跳过的卷一张都没缩、一张参照都没进缓存——两个数与解码那一个同形（窄计数器）。
                resizes: 0,
                cached_references: 0,
                io,
                // 两遍一遍都不走，三段里只有幂等那一段有数。
                timing: VolumeTiming {
                    elapsed: wall_clock(),
                    ..timing
                },
            };
            // 跳过的卷照样报这一条：「跳过」在屏幕上不该长成「卡住」，
            // 而它带的那份报告与做了事的卷同形，攒报告的那一端不必分两种情形。
            events.volume_finished(&report);
            return Ok(Some(report));
        }
        Reuse::ByPage { retained, output } => (retained, Some(output)),
        Reuse::Nothing => (Retained::nothing(source_pages), None),
    };
    let redo = retained.redo();
    let retained_pages = retained.pages();

    // dry-run 没有第二遍，缓存于是只记账不留页：用量照旧预告得出，临时文件一个不建。
    let retention = match request.mode {
        Mode::Process => cache::Retention::Keep,
        Mode::DryRun => cache::Retention::Account,
    };
    // 缓存与那几个计数是计算层唯一共用的东西：缓存要串起来（账本只有一本，参照那个数
    // 就记在它上面），解码与缩放两个数各是一次原子加。贵的那几步——解码、缩放、判据、压缩
    // ——全在锁外。
    let cache = Mutex::new(cache::PageCache::new(request.cache_budget, retention));
    let counters = ComputeCounters::default();
    // 第一遍产出的是**输出页**：一个源页产出的那几张挨着排，卷内页序就是写出顺序。
    events.pass_started(Pass::First);
    let FirstPass {
        pages: scored,
        settled,
    } = timed(&mut timing.first_pass, || {
        let compute = Compute {
            request,
            counters: &counters,
            cache: &cache,
            fingerprint: fingerprint.as_ref(),
            candidates: &candidates,
            settles,
            events,
        };
        first_pass(&mut volume, &redo, &compute, &io)
    })?;
    if events.aborting() {
        // 中止停在第一遍的页边界上：手上这半份逐页结果连同这一卷一起丢掉。
        // 第二遍还没开始，一格 `partial` 都还没建，最终位置纹丝不动。
        //
        // 它排在下面那条 `debug_assert!` **之前**：半份结果本来就凑不齐预告的张数，
        // 而那条断言问的是「拆分与预告有没有分家」，中止不是它要抓的东西。
        return Ok(None);
    }
    // 预告的张数与真产出的张数在这里第一次同时在手上。预告是**上界**（页几何批 04 号票：
    // 一个源页产出几张由内容决定），因此比的是区间而不是等号：下界是一张源页至少出一张，
    // 上界是每张都被切开。越出这个区间说明拆分与预告分了家，而那是一种静默的错——
    // 报告照出，进度条却要么冲过头、要么停在半路。
    let at_most = redo.len() * max_outputs_per_source_page(request);
    debug_assert!(
        (redo.len()..=at_most).contains(&scored.len()),
        "第一遍产出 {} 张，而重做的源页 {} 张、上界 {at_most} 张",
        scored.len(),
        redo.len()
    );

    let (verdicts, verdict) = cost::stage(cost::Stage::Summarize, || {
        summarize_volume(&scored, request)
    });
    // 一张灰度页都没重做、却留下了页（只补透传文件、重做的只有彩页）：这一卷的候选仍是从
    // 这条路上来的——留下的页正是上一趟按这条路判的——报告说的就该是这条路，而不是
    // 「一张灰度页都没有」（two-pass-rework/14）。整卷重做的卷这里是 `None`，不动。
    let verdict = verdict.or_else(|| {
        (retained_pages > 0)
            .then(|| walks.verdict_by_itself())
            .flatten()
    });
    // 留下的与重做的按阅读顺序交错成第二遍要写的那一串（two-pass-rework/14）。
    let slots = in_reading_order(&volume, &retained, &scored, &verdicts)?;
    // 真正产出的那批成员名在这里第一次齐了：加了序号的名字可能撞上卷里本来就有的成员
    // （源里同时有 `001.jpg` 与 `001-1.png`），而那一撞要在写出第一个字节之前拦下。
    // 留下的页照样在这一批里：新切出的一张与留下的一张撞名，同样不能静默覆盖。
    ensure_no_two_outputs_collide(&volume, &slots)?;
    // 第一遍提前编好字节的那两条路各自也定了一份档，而字节已经照它编好了
    // （默认那条路与顶死那一条，见 [`first_pass_verdicts`]）。
    // 两份必须逐格相同：报告说的那一档与写出去的那一页，一处出处。
    debug_assert!(
        settled.as_ref().is_none_or(|settled| *settled == verdicts),
        "第一遍编字节用的档与汇总定的档分了家：{settled:?} 对 {verdicts:?}"
    );
    // 卷内统一尺寸数的是**整本书**：留下的页也在分母里，只重做一页时众数不该由那一页说了算。
    let uniform = uniform_size(
        slots.iter().filter_map(Slot::size),
        request.profile.panel().resolution,
    );
    // 有一页失败，整卷就去隔离目录；另一个去处留着的那一份这一趟碰都不碰。
    let (output, elsewhere) = if scored.iter().any(OutputPage::failed) {
        (isolated, clean)
    } else {
        (clean, isolated)
    };
    let superseded = superseded(&elsewhere);

    // **这一卷的报告拼两次，拼法只有这一处。**一次在下面那个决策点上——交给观察者的就是它
    // （停车场 Q52：不给它，要在那里等人拿主意的调用方屏上画不出任何东西）；
    // 一次在这一卷收摊时。两次之间夹着第二遍，因此逐页那一步是**借着算**的
    // （见 [`OutputPage::to_report`]），差的只有交进来的那份计时。
    // 各拼各的话，屏上那一份与最终报告迟早会分家。
    //
    // 读缓存那两个数要的那把锁**掐在这个闭包里，而且掐在自己那个块里**：拼完就要把这份报告
    // 交给观察者，而观察者可能很久不返回（见 `progress` 的模块文档）。两个数一并读回来，
    // guard 出了那个块就没了，因此走到下面那个决策点时手上已经空了。
    // **两个数不许各锁各的**：`MutexGuard` 的临时量活到整条语句末尾，摆进结构体字面量里
    // 就是同一条线程连着锁两次——当场死锁。
    // 全卷最容易踩的就是这一处，现在不再只靠人核——[哨兵](progress::LockSentinel)守着它。
    let assemble = |timing: VolumeTiming| {
        cost::stage(cost::Stage::Assemble, || {
            let (usage, cached_references) = {
                let cache = lock(&cache);
                (cache.usage(), cache.references())
            };
            VolumeReport {
                volume: volume.root.clone(),
                output: output.clone(),
                superseded: superseded.clone(),
                pages: scored
                    .iter()
                    .zip(&verdicts)
                    .map(|(page, verdict)| page.to_report(&output, *verdict, uniform))
                    .collect(),
                retained_pages,
                source_pages,
                verdict,
                cache: usage,
                extracted,
                decodes: counters.decoder.decodes(),
                resizes: counters.resampler.resizes(),
                cached_references,
                io: io.clone(),
                timing,
            }
        })
    };

    // **续做的决策点就在这一句上**（ADR 0012 决定第 2 条）：汇总已经做完、第二遍还没开始。
    // 三个字各有一种去处，`match` 因此穷尽写开——`Instruction` 不非穷尽，多一级的那一天
    // 这里当场编译不过，而那正是要的（ADR 0013 拍死了三级）。
    // 它答的是**当场那个字**而不是闩，为什么，见 `progress::Events::ask_before_the_second_pass`。
    let walks_the_second_pass = if writes {
        match events.ask_before_the_second_pass(|| {
            assemble(VolumeTiming {
                elapsed: wall_clock(),
                ..timing
            })
        }) {
            // 答继续：往下做。参照还在缓存里，第一遍不重算——那正是续做买的东西。
            Instruction::Continue => true,
            // 答收尾：**停在这儿**。那一卷等于走了一次试算，输出一个字节都不写、报告照出
            // （见本函数的《收尾：停在决策点上》）。
            Instruction::Finish => false,
            // 答中止：这一卷等于没做，与页边界上按下它一个待遇（见《中止：回 `None`》）。
            // 一格 `partial` 都还没建，最终位置纹丝不动。
            Instruction::Abort => return Ok(None),
        }
    } else {
        // dry-run 一个文件都不落盘，第二遍无从谈起，也就没有「还做不做」可问：
        // 决策点连报都不报（spec 的 story 6）。
        false
    };
    // 建容器与收尾改名一并掐在这一段里：它们是「写出」这件事的两头（加固批 11 号票）。
    if walks_the_second_pass {
        timed(&mut timing.second_pass, || -> Result<()> {
            let mut sink = Sink::create(&output, volume.container, lodgers)?;
            let recorder = fingerprint
                .as_ref()
                .map(|fingerprint| Recorder::new(fingerprint, driver(verdict)));
            let encode = Encode {
                uniform,
                cache: &cache,
                recorder: recorder.as_ref(),
            };
            second_pass(&slots, &encode, &mut sink, prior_output.as_mut(), events)?;
            // 留下的页都搬完了，上一趟的输出放掉：归档那一支握着最终位置上那个文件的句柄，
            // 收尾改名之前必须放（见 [`sink::Written`]）。
            drop(prior_output.take());
            for extra in &volume.extras {
                // 透传文件也是第二遍写出的成员，页边界那个检查点照样在循环头上。
                if events.aborting() {
                    break;
                }
                let bytes = volume.reader.read(extra)?;
                sink.write_extra(&extra.relative, &bytes)?;
                events.step();
            }
            if events.aborting() {
                // **中止：不收尾。** `sink` 在这里走出作用域，它那格 `partial` 由析构丢掉
                // （见 `crate::sink` 的两个 `Drop`）——收尾改名是最终位置唯一被碰到的那一步，
                // 不走它，最终位置上就一个字节都没动过（ADR 0013 决定第 2 条）。
                return Ok(());
            }
            sink.finish()
        })?;
        // 闭包里那一次问的是「收不收尾」，这一次问的是「这一卷算不算做完」——
        // 两个不同的问题，各在自己那一层。再问一次恒得同一个答案，闩只升不降。
        if events.aborting() {
            return Ok(None);
        }
    }

    let report = assemble(VolumeTiming {
        elapsed: wall_clock(),
        ..timing
    });
    events.volume_finished(&report);
    Ok(Some(report))
}

/// 这一卷在另一个去处留着的上一趟输出，没有就是 `None`（12 号票的「过期副本」）。
///
/// 只问在不在，不去读它，也**不删它**：那是用户手上一份真实存在的输出，
/// 而 tonefit 在别处一律不做破坏性动作。删不删由用户定，报告负责让他知道有这么一份。
fn superseded(elsewhere: &Path) -> Option<PathBuf> {
    elsewhere.exists().then(|| elsewhere.to_path_buf())
}

/// 卷内统一的那个尺寸：失败页按它留白占位（12 号票：卷内尺寸保持一致）。
///
/// 取处理成了的那些页里**出现次数最多**的那个尺寸，并列时取先出现的。漫画卷内绝大多数页
/// 同一个尺寸，众数因此就是「这一卷看上去的样子」；取最大值会让一张跨页把整卷的占位页撑宽，
/// 取第一页则会被卷首的封面或彩页带偏。
///
/// 一页好页都没有的卷退到面板分辨率：卷内没有可参照的尺寸了，那就照这块面板的满幅出。
///
/// 数的是**输出页**：一个源页产出的那几张各有各的尺寸（页几何批 03 号票），众数因此在切开之后取。
/// **留下的页也数**（two-pass-rework/14）：它们的尺寸记在上一趟的记录里，与重做的那几张一起
/// 按阅读顺序喂进来——整本书的众数，不是这一趟碰过的那几张的众数。
fn uniform_size(sizes: impl Iterator<Item = Size>, panel: Size) -> Size {
    let mut counted: Vec<(Size, usize)> = Vec::new();
    for size in sizes {
        match counted.iter_mut().find(|(seen, _)| *seen == size) {
            Some((_, count)) => *count += 1,
            None => counted.push((size, 1)),
        }
    }
    counted
        .into_iter()
        // 并列时留先出现的那个：`max_by_key` 留的是最后一个。
        .reduce(|best, next| if next.1 > best.1 { next } else { best })
        .map_or(panel, |(size, _)| size)
}

/// 一张纸白的页：失败页留在输出里的那个**占位页**。
///
/// 白而不是别的什么——占位页顶住页序与尺寸，但不冒充内容，也不该往页上添一笔本来没有的墨。
/// 它认得出来的地方在别处：这一卷在隔离目录里，这一页的记录写着 `failed`（见 `metadata`），
/// 报告里逐条列着原因。
fn placeholder(size: Size) -> GrayImage {
    let pixels = vec![u8::MAX; size.width as usize * size.height as usize];
    GrayImage::new(size, pixels)
}

/// 汇总：先逐页定档，再把它们收成卷级的一个基准档（ADR 0006：位深按卷取上包络并加迟滞）。
///
/// 夹在两遍之间——要看完整卷才做得了，而第二遍此刻已经不必回头碰源页（ADR 0005）。
/// 返回的逐页判定与 `pages` 等长同序，第二遍读的就是它。
///
/// **只有灰度路径上的页进来。**另外两种页没有判据曲线：彩色分支上的页不该有——ADR 0006
/// 决定第 5 条说彩页在彩色 profile 下「根本不进灰度上包络」；失败页则是没有可求判据的像素
/// （12 号票）。两者在返回的判定里都占位为 `None`，位置留着——第二遍与报告都按页序取。
///
/// **第一刀按几何门切**（ADR 0007 决定第 2 条）。门成立的页与门不成立的页候选集不是同一套：
/// 后者少了抖动那一维，而上包络取的是 (位深, 抖动模式) 这个组合——候选集不同的页
/// 排不进同一条序列。卷级那一层因此只在其中**一组**上做，其余页取门成立的那一组；
/// 那一组一页都没有时才轮到另一组，摘一页是为了护着别人，而那时没有别人可护
/// （ADR 0007 决定第 5 条）。
///
/// 摘出去的那一组**不单独定档**：它们跟着卷级基准档的位深走、不低于它，抖动关掉
/// （ADR 0007 决定第 3 条）。门只拿走抖动，不拿走档次——让它们各按自己那条曲线定，
/// 位深那一维也会跟着逐页变，而它们并没有偏离卷内分布，摘它们的理由是几何。
/// 反过来只给基准档也不行：抖动被拿走之后同一档位深保真更差，那一页可能真的还要高一档。
///
/// **第二刀按页残缺切**（04 号票）。部分救回页有判据曲线，那条曲线却是在一页大半留白的图上
/// 求出来的，代表不了这一卷。它因此不进上包络，按自己那条曲线单独定档——与特例页同一个待遇
/// （ADR 0006 决定第 5 条），只是摘它的理由是页残缺，不是判据偏离。
/// 一页不剩地落在救回那一侧时一页都不摘：其余页不能空着，与门那一刀同一条规矩
/// （也与 `envelope::summarize` 里「一页不剩地落到特例侧」同一条）。
///
/// **两刀落在同一页上时，门那一刀在外层**（ADR 0007 决定第 3 条）：既没解全、又贴不住面板的页
/// 拿的是「基准档的位深，不低于它，抖动关掉」，不是 04 号票那条「按自己那条曲线单独定档」。
/// 摘部分救回页的理由是它那条曲线不具代表性——一页大半留白，误差恒为零，判出来必偏低——
/// 而不具代表性的曲线更没有资格把这一页压到基准档以下。
///
/// 逐页定档也落在这里，而不在第一遍：摘出去的那两组都要拿卷级基准档当参照，
/// 而那一档要看完整卷才定得下来。
///
/// 两条出口走不到上包络，各有各的道理：默认那条路上位深逐页各判各的，上包络要显式
/// `--envelope` 才在场（ADR 0018 决定第 2、5 条）；覆盖项裁到只剩一个候选是判定整个被顶掉、
/// 逐页已全是 `Override`——后者不是「没开」，而是逐页结果里根本没有分布可聚合。
/// 两者在报告里各说各的，见 [`VolumeVerdict`]。
///
/// 进来的是**输出页**，不是源页（页几何批 03 号票）：一个源页产出的那几张各有各的几何、各有各的
/// 判据曲线，卷级那一层因此在切开之后取——序号也都指进输出页那个序列，
/// 上包络的定档页序号跟着（见 [`Envelope::driver`]）。
fn summarize_volume(
    pages: &[OutputPage],
    request: &Request,
) -> (Vec<Option<Verdict>>, Option<VolumeVerdict>) {
    // 灰度路径上那些页在 `pages` 里的序号。卷级的一切都只在它们身上做。
    //
    // 序号非带不可：卷级每一步都只在其中一部分页上做——彩页与失败页根本不在场，
    // 门不成立的页与部分救回页各自摘出去——手上那个序列与卷内页序早就不重合了。
    let gray: Vec<usize> = pages
        .iter()
        .enumerate()
        .filter(|(_, page)| page.scores().is_some())
        .map(|(index, _)| index)
        .collect();
    // 门先分组。两组的候选集不是同一套，混不得（见 [`Candidates`]）。
    let (holding, broken): (Vec<usize>, Vec<usize>) = gray
        .iter()
        .copied()
        .partition(|&index| pages[index].gate() == Some(GeometryGate::Holds));
    // 一页门成立的灰度页都没有时，不成立的那些页就当这一卷的其余页，基准档由它们定出
    // ——那一档必然不抖（ADR 0007 决定第 5 条）。
    let (inside, outside) = if holding.is_empty() {
        (broken, Vec::new())
    } else {
        (holding, broken)
    };

    let mut verdicts: Vec<Option<Verdict>> = vec![None; pages.len()];
    // 一张灰度页都没有的卷没有候选可判：只装着彩页的、整卷全失败的，都是这一支。
    let Some(&first) = inside.first() else {
        return (verdicts, None);
    };
    let scores = |index: usize| pages[index].scores().expect("灰度路径上必有判据曲线");

    let threshold = request.profile.threshold();
    // 「覆盖项裁到只剩一个候选」问的是**其余页那一组**的候选集：门那两组不一样长，
    // 拿门不成立的页去问，答案会随卷里第一张灰度页碰巧是哪一种而变。
    let pinned = pinned(request, scores(first));
    // 逐页先各判各的。摘出去的两组都还用得上自己这一档：部分救回页直接用它，
    // 门不成立的页拿它跟基准档比出更严的那个（ADR 0007 决定第 3 条）。
    for &index in &gray {
        verdicts[index] = Some(decide::decide(scores(index), threshold, pinned));
    }

    if let Some(candidate) = pinned {
        return (verdicts, Some(VolumeVerdict::Override(candidate)));
    }
    if !request.envelope {
        // 默认那条路：每一页拿到判据说它要的那一档，卷级一层都没有（ADR 0018 决定第 2 条）。
        // 上面那一刀分的两组、摘出去的部分救回页，在这里都不必再问——没有基准档要定，
        // 也没有邻居要比。`verdicts` 里已经是逐页判定。
        return (verdicts, Some(VolumeVerdict::PerPage));
    }

    // 上包络只在其余页那一组的完好页上取（04 号票）。两条出口上不分这一刀：覆盖项顶掉了判定、
    // 默认那条路上卷级那一层根本不在场，两种情形下都没有一个「卷级的档」可供谁去污染。
    // 摘出去的部分救回页留着逐页判定：`verdicts` 里已经是它了，不必再写一遍。
    let (body, salvaged): (Vec<usize>, Vec<usize>) = inside
        .iter()
        .copied()
        .partition(|&index| !pages[index].salvaged());
    // 一页不剩地落在救回那一侧时一页都不摘：其余页不能空着。
    let body = if body.is_empty() { salvaged } else { body };

    let inputs: Vec<envelope::Page> = body
        .iter()
        .map(|&index| envelope::Page {
            scores: scores(index),
            decided: verdicts[index].expect("灰度页都判过了").candidate,
        })
        .collect();
    let envelope::Summary {
        envelope,
        verdicts: refined,
    } = envelope::summarize(&inputs, threshold).expect("其余页非空");
    for (&index, verdict) in body.iter().zip(refined) {
        verdicts[index] = Some(verdict);
    }
    // 门不成立的页：跟着基准档的位深走、不低于它，抖动关掉（ADR 0007 决定第 3 条）。
    // 它们与 `body` 不相交，逐页那一档因此还在原处等着被读。
    for &index in &outside {
        let own = verdicts[index].expect("灰度页都判过了").candidate.bit_depth;
        verdicts[index] = Some(Verdict {
            candidate: Candidate::new(envelope.base.bit_depth.max(own), Dither::Off),
            reason: Reason::OutsideTheGate,
        });
    }
    // 定档页的序号在上包络那一侧指进**其余页**的序列，报告里那个序号指进整卷的页。
    // 卷内混着彩页、门不成立的页或部分救回页时两者不重合，这一步把它换回去——不换，
    // 报告会指着另一页说「就是它定的档」。
    let envelope = Envelope {
        driver: body[envelope.driver],
        ..envelope
    };
    (verdicts, Some(VolumeVerdict::Envelope(envelope)))
}

/// 覆盖项裁到只剩一个候选时的那一个：判定被顶掉，判据说什么都不改变结果（spec 的 story 23）。
///
/// 「裁到只剩一个」与「有覆盖项」两条都要：`--gray-levels 2` 撞上几何门不成立同样只剩一个候选，
/// 但那一档是判出来的，不是被顶掉的——理由分得清，报告才解释得了它是怎么来的。
///
/// 反过来，只点了一维的覆盖项裁不到只剩一个：`--bit-depth 4` 而其余页那一组的门开着时，
/// 抖动那一维还有得判，判据照旧说了算。
///
/// `scores` 取的是**其余页那一组**里的一页（见 [`summarize_volume`]）。裁到只剩一个的
/// 覆盖项落在门不成立那一组上时，那一组的候选集必然也只剩同一个——门只拿走抖动，
/// 而剩下的那一个既然过得了门，它本来就不抖。
fn pinned(request: &Request, scores: &[CandidateScore]) -> Option<Candidate> {
    let overridden = request.bit_depth.is_some() || request.dither.is_some();
    match scores {
        [only] if overridden => Some(only.candidate),
        _ => None,
    }
}

/// 第一遍产出的一张**输出页**：它从哪一个源页来、写到哪儿去，有没有处理成，
/// 以及处理成了的话留下了什么。
///
/// 一个源页产出**一到多张**（见 [`split`]）。因此 `source` 会重复
/// ——同一源页切出来的几张都指着它——而 `target` 一定不重复
/// （见 [`ensure_one_member_per_output`]）。第一遍之后管线上处处按输出页数事：
/// 汇总、上包络的序号、第二遍的写出、报告里的逐页结果，一律以它为单位。
struct OutputPage {
    /// 这一张来自哪一个源页：卷根接上成员相对路径，报告与错误信息用它指人。
    source: PathBuf,
    /// 它在输出容器里的相对位置（见 [`output_name`]）。
    target: PathBuf,
    /// 它的**来路**：来自哪个源成员、在那一族里排第几、那一族共几张（页几何批 04 号票）。
    ///
    /// 与 [`source`](Self::source) 不是重复：那一项是**给人读的身份**（卷根接上相对路径，
    /// 报告与错误信息指人用它），这一项是**写进 tEXt 的索引**（卷内相对路径，转义成 ASCII，
    /// 带着那一族的位次）。幂等靠它把输出页反查回源页——一个源页产出几张由内容决定，
    /// 输出成员名因此在碰像素之前预告不出来（见 [`Origin`] 与 [`compare_with_the_prior_output`]）。
    ///
    /// **关掉记录的那一趟它整个不在**（07 号票）：它唯一的消费者是 [`Recorder`]，
    /// 而 `--no-metadata` 那一趟一个 [`Recorder`] 都不在场——既没有记录可写，
    /// 也没有依据可比。在场与否与指纹同一格，见 [`Placement::new`]。
    origin: Option<Origin>,
    outcome: Outcome,
}

/// 一页在第一遍的结局。
///
/// 与报告那一侧的 [`PageOutcome`] 同形而不同物：这里装的是**第二遍要用的东西**
/// （缓存序号、编好的字节），那里装的是报告要读的东西。两者各留各的，
/// 内部产物才不会跟着报告一路公开出去。
enum Outcome {
    /// 处理成了的一页：完好的，或者救回来一段的。
    Processed {
        size: Size,
        /// 这一页裁掉了多少白边（页几何批 02 号票）。裁边在适配之前，`size` 由裁完的尺寸算出。
        crop: Crop,
        /// `size` 是不是**兜底上界**改出来的（07 号票，见 [`FitMode::target`]）。
        ///
        /// 它跟着页走，不由报告那一侧按尺寸倒推：算目标尺寸的地方只有一处，
        /// 倒推一遍就是第二处——与几何门那一条同一个理由（见 [`Branch::Gray`] 的 `gate`）。
        backstopped: bool,
        /// 这一张是那一刀的产物：切在哪条装订沟上、是哪一侧。整页出的是 `None`（04 号票）。
        cut: Option<spread::Cut>,
        /// 这一张所属的源页够得上**跨页候选**吗（04 号票）。与 `cut` 一起才说得全
        /// 拆分那两级，见 [`PageReport::spread_candidate`]。
        spread_candidate: bool,
        scaling: resample::Scaling,
        color: PageColor,
        branch: Branch,
        /// 这一页救回了多少。整解出来的完好页是 `None`（04 号票，见 `decode`）。
        salvage: Option<Salvage>,
    },
    /// 失败页：字节读不出来，或者连完整尺寸都解不出来（12 号票）。
    ///
    /// 它在这里仍然占着自己那一格——页序不因为一页坏了就错位，
    /// 第二遍照样给它写一张卷内统一尺寸的白页。
    Failed { reason: String },
}

/// 一页在第一遍里走的那条分支，连同它留给第二遍的东西。
///
/// 两条分支留下的不是同一套：灰度路径留判据曲线与缓存序号，彩色分支留编好的字节
/// （ADR 0005 决定第 4 条）。
enum Branch {
    /// 灰度路径。
    Gray {
        scores: Vec<CandidateScore>,
        /// 这一页的几何门判定（ADR 0007 决定第 1 条：门逐页判）。
        ///
        /// 它跟着页走，不由汇总那一步按尺寸重算：判定几何门的地方只有一处
        /// （[`GeometryGate::of`]，ADR 0003 要求灰阶硬上界与抖动判定同源），
        /// 而重算一遍就是第二处。
        gate: GeometryGate,
        /// 这一页在缓存里的序号。
        ///
        /// 序号跟着页走，不由第二遍数数补出来：彩页在彩色 profile 下不进灰度缓存
        /// （ADR 0005 决定第 4 条）、失败页也不进，页序与缓存序因此不重合，
        /// 而重新数出来的序号会静默地把另一页的像素写到这一页的位置上。
        slot: usize,
        /// **纸白对齐对这一页做了什么**（纸白对齐批 02 号票）。
        ///
        /// 它跟着页走，不由报告那一步按像素重算：量纸白的地方只有一处
        /// （[`align_white`]），重算一遍就是第二处——而对齐过的图上
        /// 重算出来的答案还是错的（那时纸白已经是 255 了）。
        white: WhiteAlignment,
    },
    /// 彩色分支：第一遍缩放并编好的 PNG 字节，等写出那一遍按阅读顺序落位。
    ///
    /// dry-run 没有写出那一遍，也就不编——一个字节都不留（spec 的 story 6）。
    Color { encoded: Option<Vec<u8>> },
}

impl OutputPage {
    /// 这一页的判据曲线。彩色分支与失败页上都没有——一条不量化，一条没解出来。
    fn scores(&self) -> Option<&[CandidateScore]> {
        match &self.outcome {
            Outcome::Processed {
                branch: Branch::Gray { scores, .. },
                ..
            } => Some(scores),
            _ => None,
        }
    }

    /// 这一页写出的尺寸。失败页没有自己的尺寸——它按卷内统一尺寸出，而那个数
    /// 恰恰是从这个函数的结果里算出来的（见 [`uniform_size`]）。
    fn size(&self) -> Option<Size> {
        match &self.outcome {
            Outcome::Processed { size, .. } => Some(*size),
            Outcome::Failed { .. } => None,
        }
    }

    /// 这一页为什么失败，没失败就是 `None`。
    ///
    /// 事件流报「一页失败了」那一条用它取原因（见 [`Compute::page`]）。
    fn failure(&self) -> Option<&str> {
        match &self.outcome {
            Outcome::Failed { reason } => Some(reason),
            Outcome::Processed { .. } => None,
        }
    }

    /// 这一页失败了吗。一卷里只要有一页答是，整卷就进隔离目录。
    ///
    /// 问的是 [`failure`](Self::failure)：「失败了吗」与「为什么失败」只有一个出处，
    /// 两处各自 `matches!` 一遍，将来多一种失败就会有一处忘了改。
    fn failed(&self) -> bool {
        self.failure().is_some()
    }

    /// 这一页的几何门判定。判定范围之外的页没有——彩色分支上的页不在范围内
    /// （ADR 0010 决定第 4 条），失败页连几何都没有。部分救回页在范围内（ADR 0007 决定第 1 条）。
    fn gate(&self) -> Option<GeometryGate> {
        match &self.outcome {
            Outcome::Processed {
                branch: Branch::Gray { gate, .. },
                ..
            } => Some(*gate),
            _ => None,
        }
    }

    /// 这一页是救回来的吗（04 号票）。答是的页不参与卷级上包络。
    fn salvaged(&self) -> bool {
        matches!(
            self.outcome,
            Outcome::Processed {
                salvage: Some(_),
                ..
            }
        )
    }

    /// 补上汇总定下的那个判定，就是报告要的一页。缓存序号与编好的字节都不进报告——
    /// 它们是管线内部的事。
    ///
    /// `output` 是这一卷的去处，接上这一张的成员名就是它写出去的位置。
    /// `uniform` 只对失败页说话：它写出去用的就是这个尺寸。
    /// **借着算，不吃掉这一页**：同一批页要拼两次报告——一次在[决策点](progress::Events::ask_before_the_second_pass)
    /// 上（那一份交给观察者拿主意，停车场 Q52），一次在这一卷收摊时——而第二遍夹在两者中间，
    /// 它读的是这同一批页。拿走所有权的话，第一次拼完第二遍就没得读了。
    ///
    /// 复制掉的只有报告要的那几格（源路径、判据曲线、失败那句话）：编好的字节与缓存序号
    /// 是管线内部的东西，本来就不进报告，因此这一份**不含**页像素那一侧的任何东西。
    fn to_report(&self, output: &Path, verdict: Option<Verdict>, uniform: Size) -> PageReport {
        let output = output.join(&self.target);
        let (size, outcome) = match &self.outcome {
            Outcome::Processed {
                size,
                crop,
                backstopped,
                cut,
                spread_candidate,
                scaling,
                color,
                branch,
                salvage,
            } => {
                let processed = Processed {
                    crop: *crop,
                    backstopped: *backstopped,
                    cut: *cut,
                    spread_candidate: *spread_candidate,
                    scaling: *scaling,
                    color: *color,
                    branch: match branch {
                        Branch::Gray {
                            scores,
                            gate,
                            white,
                            ..
                        } => PageBranch::Gray {
                            scores: scores.clone(),
                            verdict: verdict.expect("灰度路径上必有判定"),
                            gate: *gate,
                            white: *white,
                        },
                        Branch::Color { .. } => PageBranch::Color,
                    },
                };
                let outcome = match salvage {
                    Some(salvage) => PageOutcome::Salvaged {
                        page: processed,
                        salvage: *salvage,
                    },
                    None => PageOutcome::Whole(processed),
                };
                (*size, outcome)
            }
            Outcome::Failed { reason } => (
                uniform,
                PageOutcome::Failed {
                    reason: reason.clone(),
                },
            ),
        };
        PageReport {
            source: self.source.clone(),
            output,
            size,
            outcome,
        }
    }
}

/// 本次的两套候选：几何门成立的那一套，与门不成立的那一套。
///
/// 两套在碰卷之前就备好，页判出门之后现取一套（[`Candidates::for_gate`]）。
/// 门逐页判，一卷里两套都用得上——混排卷正是 06 号票要收的那个形态（ADR 0007 决定第 1 条）。
///
/// 门不成立那一套是成立那一套的**子集**：同样的位深，少了抖动那一维。
/// 「候选集全卷同一套」因此只在**一组之内**成立，而卷级那一层只在其中一组上做
/// （见 [`summarize_volume`]）。
struct Candidates {
    /// 门成立时的候选集。它非空——覆盖项把它裁空的话，整趟在碰卷之前就被拒了
    /// （见 [`ensure_the_overrides_leave_a_candidate`]）。
    holds: Vec<Candidate>,
    /// 门不成立时的候选集。覆盖项把它裁空时是 `None`：`--dither fs` 撞上一页贴不住面板
    /// 就是这个局面，那正是互锁 ③。这一格留到真撞上那一页时才说话——门是**页**的事实，
    /// 一卷里可能一页都不撞。
    ///
    /// **它不装那句拒绝**（从前装的是一个碰卷之前备好的 `Err`）：对用户说什么要
    /// **这一页**才说得全——够得着以高为准那条出路的页与够不着的页听见的不是同一句
    /// （21 号票，停车场 Q102）。那一句因此由 [`for_gate`](Self::for_gate) 现造。
    broken: Option<Vec<Candidate>>,
}

impl Candidates {
    fn new(request: &Request) -> Result<Self> {
        Ok(Self {
            holds: candidates(request, GeometryGate::Holds)?,
            // 丢掉的那个错误是[规则那一句](Interlock::DitherOutsideTheGate)，出路那一半
            // 还没有页可判（见 [`why_nothing_is_left`]）——这里只要「裁空了没有」。
            // 位深那一维在上一行就拦下了（它不看门），走到这里的 `Err` 只可能是互锁 ③。
            broken: candidates(request, GeometryGate::Broken).ok(),
        })
    }

    /// 门是这个结果的页该拿哪一套。
    ///
    /// 裁空那一支上**当场造那句拒绝**，而不是重说一遍备好的那一份：撞上门的页可能有
    /// 好几张，而**每一张听见的不是同一句**——出路由这一页的几何定（21 号票）。
    /// 判据只有一问：**换成以高为准之后，这一页的门成不成立**。成立就指得出那条出路；
    /// 不成立的只有一种页——以高为准算出的目标尺寸越过[兜底上界](max_target_pixels)、
    /// 被退回 fit-inside 的那种（07 号票），对它劝换适配方式是假话。
    ///
    /// 判定本身**不在这里**：那一问住在几何那一层
    /// （[`geometry::holds_by_height`]，门与目标尺寸各自的唯一出处都在它里面），
    /// 这一处只是问它一句、把答案交给措辞。收源尺寸与面板而不是收一个算好的布尔，
    /// 是为了让门成立那一支**一分钱都不花**。
    ///
    /// 造出来的那一份戴着 [`Refusal`]：这一支的处置是「维持拒绝」（互锁 ③），
    /// 摘掉标记它就降级成了「这一卷没做成」，而 `--dither fs` 对每一卷都错。
    fn for_gate(&self, gate: GeometryGate, source: Size, panel: Size) -> Result<&[Candidate]> {
        match gate {
            GeometryGate::Holds => Ok(&self.holds),
            GeometryGate::Broken => self.broken.as_deref().ok_or_else(|| {
                Refusal(dither_outside_the_gate_error(geometry::holds_by_height(
                    source, panel,
                )))
                .into()
            }),
        }
    }
}

/// 第一遍：读 → 解码 → **切开** → 逐张彩页识别 → 分流。
///
/// 灰度路径：转灰 → 几何与几何门 → 缩放 → 判据曲线 → 进缓存。
/// 彩色分支：几何 → 缩放 → 编码，不进缓存、不求判据（ADR 0005 决定第 4 条）。
///
/// **灰度页在这一遍就编完的有两条路**（那一格装的因此是字节，不是参照，见 [`Settles`]）：
/// 默认那条路上位深逐页各判各的，判据一出来这一页的档就定了，当场量化、编码
/// （12 号票；ADR 0018）；覆盖项把候选裁到只剩一个的那一趟判定在碰卷之前就定死，
/// 同样当场编（06 号票）。两条路上参照一张都不进缓存。
/// 编好的字节进的是缓存，**这一遍仍旧一个字节都不写出去**。
///
/// **产出的是输出页，不是源页**（页几何批 03 号票）：一张源页读一次、解一次，切成一到多张
/// （见 [`split`]），此后每一张各走各的分支、各占一个缓存序号、
/// 各占报告里的一格。同一源页切出来的那几张挨着排，卷内的输出页序因此仍是阅读顺序。
///
/// **识别排在转灰之前**，因为转过之后就没有颜色可看了；也排在汇总之前，
/// 因为分流决定了哪些页进得了上包络（ADR 0006 决定第 5 条）。
/// 走哪条分支由**面板与页**共同决定：只有彩色面板上的彩页走彩色分支，
/// 黑白面板上的彩页转灰、和其它页走同一条路。
///
/// 判据两种模式都求值，dry-run 预告的就是照做时的那一档（spec 的 story 6）。
/// 覆盖了判定也照求：`--dry-run --bit-depth 2` 要说得清「你点的这一档判据是多少」。
/// 彩色分支上没有这回事——那条路径不量化，dry-run 因此连编码都省了。
///
/// **几何门在这一遍上逐页收口。**门是几何的、一页看得出来，而它只决定这一页
/// （ADR 0007 决定第 1 条）：算到哪一页就判哪一页的门，候选集随之定下，判据只在那一套上求。
/// 一页贴不住面板不再改变别的页求几个候选，收尾处因此不必回头统一裁一遍——
/// 而从前那一裁，正是「一页否决整卷」在实现上的落点。
///
/// **彩色分支上的页不在判定范围内。**门撑的是抖动与面板灰阶那道硬上界（ADR 0007、ADR 0003），
/// 两者都只作用在灰度路径上；彩页既不量化也不抖动，它的几何事实对那两件事没有说话的资格。
///
/// **部分救回页在范围内**：它的尺寸是文件头里的真尺寸，答得出「这一页会不会被下游再缩一次」，
/// 而它答的只是自己那一页。04 号票把它摘出去，是因为那时门对整卷只有一个结果；
/// 门改成逐页判之后那条理由不在了，口径把它收了回来（ADR 0007 决定第 1 条）。
///
/// **读不出、解不出的页在这里变成失败页**（12 号票），而不是让整卷的调用返回 `Err`。
/// 它同样不在判定范围内，理由比彩页还直白：它连尺寸都没有。判据与缓存也一样绕开——
/// 没有像素可求判据，也没有像素可缓存。它留下的只有一条原因，等第二遍给它留一张白页。
///
/// **读取与计算在这里分成两层**（13 号票，见 `read` 与 `medium`）：读取按介质定并发度，
/// 计算走 rayon 满核，两层之间是一道按在途字节背压的有界通道。页因此**乱序算完**，
/// 页序在收尾处按序号归位——除此之外，这一遍的产物与一页一页顺着做完全相同。
///
/// **中止让它回一份不全的清单**（ADR 0013 决定第 2 条）：发页那一侧的循环头上问一次闩，
/// 答中止就不再往下发。回来的因此可能短于源页数——[`process_volume`] 紧接着问一次闩，
/// 是中止就把整卷丢掉，那半份清单谁也看不见。
fn first_pass(
    volume: &mut Volume,
    redo: &[usize],
    compute: &Compute,
    io: &IoPlan,
) -> Result<FirstPass> {
    let Compute {
        request,
        settles,
        events,
        ..
    } = compute;
    let (settles, events) = (*settles, *events);
    // 页的身份先取出来：读取层要借走 `reader`，此后就没有一个完整的 `Volume` 可问了。
    //
    // **只走 `redo` 点名的那些源页**（two-pass-rework/14）：留下的页不读、不解、不判、不编，
    // 它们的字节第二遍从上一趟的输出里搬。整卷重做时 `redo` 就是全部源页。
    let sources: Vec<PathBuf> = redo
        .iter()
        .map(|&index| volume.identity(&volume.pages[index]))
        .collect();
    let Volume { pages, reader, .. } = volume;
    let members: Vec<&Member> = redo.iter().map(|&index| &pages[index]).collect();

    let mut scored: Vec<(usize, Result<Vec<OutputPage>>)> =
        read::reads(reader, &members, io.readers.count, read::BUDGET)
            // **页边界那个检查点**（ADR 0013 决定第 2 条）：中止就不再往下发页。
            // 它拦在 `par_bridge` **之前**，因此停下来的不止计算层——读取层的发号闸
            // 跟着关上，那几条读取线程当场收摊（见 `read::Throttle::stop`）。
            // 停在这儿手上只有半份逐页结果，[`process_volume`] 随后连同整卷丢掉。
            .take_while(|_| !events.aborting())
            .par_bridge()
            .map(|read| {
                let index = read.index;
                // 成员表在这一层照旧读得到：读取层借的也是共享引用。
                // 输出成员名由相对路径推出（见 [`output_name`]）。
                let relative = &members[index].relative;
                (
                    index,
                    compute.page(redo[index], &sources[index], relative, read.bytes),
                )
            })
            .collect();
    // 计算层乱序完成，页序在这里归位。往后每一处「第 n 页」都指得回同一页。
    scored.sort_by_key(|(index, _)| *index);
    // 归位**之后**才短路取错，因此报出来的是序号最小的那一页出的错，不是最先撞上的那一页：
    // 换一次调度就换一句错误的报告等于没有报告。`--dither fs` 撞上几何门那一支尤其吃这一条,
    // 那一支上没有报告可看，错误里指的那一页是唯一的线索。
    // 代价是一页出错时整卷仍会算完，而这一支上整卷本来就要作废，省下的那点算力买不到什么。
    //
    // 摊平放在最后：一个源页产出的那几张挨着排（页几何批 03 号票），卷内的输出页序因此仍然是阅读顺序。
    let pages: Vec<OutputPage> = scored
        .into_iter()
        .map(|(_, page)| page)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();
    // 第一遍就编了字节的那两条路各自定过一份档，交出去对账（见 [`first_pass_verdicts`]）。
    // 中止那一支不留——手上这半份结果连同整卷一起丢掉。
    let settled = if events.aborting() || !settles.encodes_in_the_first_pass() {
        None
    } else {
        Some(first_pass_verdicts(
            &pages,
            request.profile.threshold(),
            settles.pinned(),
        ))
    };
    Ok(FirstPass { pages, settled })
}

/// 第一遍交出来的东西：这一卷的输出页，加上**第一遍就照它编好了字节**的那份逐页判定。
///
/// 它只在字节提前编好的那两条路上有——默认那条路（逐页各判各的）与顶死那一条
/// （见 [`first_pass_verdicts`]）；上包络那条路上字节要等第二遍才编，这一格因此是 `None`。
///
/// 它**不是**报告用的那一份——报告仍旧由 [`summarize_volume`] 一处说了算。
/// 它在这里只为一件事：拿去和汇总那一份对一遍，两处一旦分家，
/// 写出的字节与报告说的那一档就对不上了。
struct FirstPass {
    pages: Vec<OutputPage>,
    settled: Option<Vec<Option<Verdict>>>,
}

/// **计算层**这一卷自己那两样带计数的家伙：解码器与缩放器（`CONTEXT.md` 的《读取层 / 计算层》）。
///
/// 装成一个而不是两个参数：它们从 [`process_volume`] 一路传到 [`Compute`]，
/// 走的是同一截路、活的是同一段命——一卷一份，卷跑完连同各自的数一起交进报告。
/// 两个都是锁外的原子加，因此满核并行照旧不必独占。
///
/// **不叫「窄计数器」**：那个词指的是三个数（`CONTEXT.md` 的《窄计数器》），
/// 而第三个不在计算层——参照进缓存那一个记在缓存上，缓存是**两遍之间**的东西，
/// 账本要串起来，另走一把锁。
#[derive(Debug, Default)]
struct ComputeCounters {
    decoder: decode::Decoder,
    resampler: resample::Resampler,
}

/// 第一遍上每条计算线程共用的那一摊。
///
/// 装成一个结构体而不是一串参数，是因为它要整个被闭包借走：拆成六个参数，
/// 闭包的捕获清单就得逐个写一遍，而漏掉一个的报错在 rayon 那一层读起来毫无线索。
/// 由 [`process_volume`] 装好交给 [`first_pass`]：候选与 [`Settles`] 在幂等那一道就要问
/// （页级依据这一趟成不成立），那两样因此在第一遍之前就备好了。
struct Compute<'a> {
    request: &'a Request,
    /// 解码与缩放两个动作，连同各自记着的那个数（见 [`ComputeCounters`]）。
    counters: &'a ComputeCounters,
    /// 缓存的账本只有一本，因此非串起来不可。压缩在锁外做（见 `cache::compress`）。
    cache: &'a Mutex<cache::PageCache>,
    fingerprint: Option<&'a Fingerprint>,
    /// 两套候选集。这一页判出门之后现取一套（见 [`Candidates::for_gate`]）。
    candidates: &'a Candidates,
    /// 这一卷的档什么时候定得下来——它决定灰度页那一格缓存里装的是什么。
    settles: Settles,
    events: progress::Events<'a>,
}

/// 这一卷的档**什么时候**定得下来。第一遍走完那一格缓存里装参照还是装编好的字节，
/// 由它一处说了算。
///
/// 三者互斥，因此是**一个枚举**而不是几个可空的字段：几个 `Option` 表示得出
/// 「逐页各判各的」与「顶死」同时在场这种组合，而那个组合不存在，读代码的人于是得
/// 自己跑去几处对一遍。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Settles {
    /// **等整卷**：卷级上包络的基准档要看完整卷才定得下（ADR 0005、ADR 0006）。
    /// 那一格装参照，第二遍取回来量化编码。
    AfterTheVolume,
    /// **这一页自己**：默认那条路上位深逐页各判各的，不做迟滞（ADR 0018 决定第 2 条），
    /// 判据一出来这一页的档就定了，没有后文可等。第一遍当场量化编码，那一格从头装的
    /// 就是字节，**参照一张都不进缓存**（ADR 0005 的《第二遍在逐页那条路上退化成纯写出》）。
    OnItsOwn,
    /// **碰卷之前就定死**：覆盖项把候选裁到只剩一个，判据说什么都不改结果。
    /// 第一遍当场量化编码，那一格从头装的就是字节，**参照一张都不进缓存**
    /// （06 号票，见 [`pinned_up_front`]）。
    UpFront(Candidate),
}

impl Settles {
    /// 这一趟走哪一条。
    ///
    /// **试算不在字节提前编好的那两条之内**：那一趟没有第二遍，编出来的字节一个读者都
    /// 没有，缓存也只记账、不留页（[`cache::Retention::Account`]）——照旧攒参照，
    /// 预告的缓存用量因此与照做那一趟不是同一个数（停车场 Q430、Q537）。
    ///
    /// 照做那一遍上先问顶死（见 [`pinned_up_front`]）。那一问有两种答不下来的方式，
    /// 两种落点相反：答得出那一档就是顶死；答不出来（要等整卷判完门）就等整卷——
    /// **默认那条路上也一样**，那一卷照旧攒整卷参照，与上包络那条路逐字节相同。
    /// 判定还有得判时才分两条路：上包络开着等整卷，不开就这一页自己。
    fn for_this_run(request: &Request, candidates: &Candidates) -> Self {
        if request.mode != Mode::Process {
            return Self::AfterTheVolume;
        }
        Self::if_processing(request, candidates)
    }

    /// **照做那一趟**走哪一条——不看这一趟的模式。
    ///
    /// 源哈希按哪种作用域算、记、比，问的是它，不是 [`for_this_run`](Self::for_this_run)
    /// （two-pass-rework/14、15）：试算要预告的是照做时会发生的事（spec 的 story 6），
    /// 而照做时一页的依据是它自己的还是全卷的，只看这一页的字节是不是只取决于它自己——
    /// 那是参数的性质，与这一趟写不写无关。试算自己那一格缓存装什么，仍由上面那一问答。
    ///
    /// 它**不是** `request.envelope`：`--envelope` 加两维都点名的那一趟顶死、字节只取决于页，
    /// 按页；只点了一维而两组门混着的那一角要等整卷才知道理由那一句，按卷（停车场 Q635、Q683）。
    /// 「依据的作用域 = 这一页的字节取决于什么」一处出处，两条路不重叠、不互相兜底。
    fn if_processing(request: &Request, candidates: &Candidates) -> Self {
        match pinned_up_front(request, candidates) {
            Some(Some(candidate)) => Self::UpFront(candidate),
            None => Self::AfterTheVolume,
            Some(None) if request.envelope => Self::AfterTheVolume,
            Some(None) => Self::OnItsOwn,
        }
    }

    /// 灰度页的字节是不是第一遍就编好：这一页自己定得下、或碰卷之前就定死，都是；
    /// 等整卷的那条路不是——那一格装参照，字节第二遍才编。
    fn encodes_in_the_first_pass(self) -> bool {
        match self {
            Self::AfterTheVolume => false,
            Self::OnItsOwn | Self::UpFront(_) => true,
        }
    }

    /// 这条路上一张灰度页都没判时该报的卷级判定：默认那条路仍是逐页，顶死的那一趟仍是覆盖——
    /// 两条路上「候选从哪来」不取决于这一趟判了几页；等整卷的那条路答不出来（基准档要判过才有）。
    /// 按页跳过留下了页、重做的里头没有灰度页时用它（two-pass-rework/14）。
    fn verdict_by_itself(self) -> Option<VolumeVerdict> {
        match self {
            Self::OnItsOwn => Some(VolumeVerdict::PerPage),
            Self::UpFront(candidate) => Some(VolumeVerdict::Override(candidate)),
            Self::AfterTheVolume => None,
        }
    }

    /// 顶死的那一档，交给 [`decide::decide`] 当 `pinned`：只有顶死的那一趟有。
    fn pinned(self) -> Option<Candidate> {
        match self {
            Self::UpFront(candidate) => Some(candidate),
            Self::AfterTheVolume | Self::OnItsOwn => None,
        }
    }
}

impl Compute<'_> {
    /// 算一张**源页**：解码 → 切开 → 每一张各自彩页识别、分流。
    /// 语义与顺着做时逐字相同，见 [`first_pass`]。
    ///
    /// 报到一步，不是几步：步按**源页**数（`CONTEXT.md` 的《进度》：第一遍走每一页），
    /// 而一张源页读一次、解一次，切成几张不改变这一遍的工作量。
    ///
    /// 失败页在这里就报出去，不等整卷跑完（09 号票：失败页出现的当场就在主区可见）。
    /// 报在这一层而不在造出它的那一层：坏字节与解不出来的图各从各的地方回来，
    /// 而两条路最后都汇到这个返回值上——报在汇合处，以后多一种失败也不会漏报。
    ///
    /// 两条报到都在解码与切分**之后**：这条线程此刻一把锁都没拿着（缓存那把在
    /// [`Compute::gray_page`] 里进出），而观察者可能很久不返回（见 `progress` 的模块文档）。
    fn page(
        &self,
        index: usize,
        source: &Path,
        relative: &Path,
        bytes: Result<Vec<u8>>,
    ) -> Result<Vec<OutputPage>> {
        let pages = self.split_and_branch(index, source, relative, bytes)?;
        for page in &pages {
            if let Some(reason) = page.failure() {
                self.events.page_failed(&page.source, reason);
            }
        }
        self.events.step();
        Ok(pages)
    }

    /// 解一张源页，**分流**，再按裁边 → 判跨页 → 拆分 → 每半再裁 → 适配走下去。
    ///
    /// 分流排在切开**之前**，也只问一次（ADR 0005 决定第 1 条：读 → 解码 → 彩页识别 →
    /// 拆分/裁边）：彩不彩是**源页**的事实，一幅跨页画不会因为从中间切开就有一半不再是彩页。
    /// 走哪条分支由**面板与页**共同决定——只有彩色面板上的彩页走彩色分支。
    ///
    /// `index` 是这一张在卷里的源页序号：盖记录时页级那一份源哈希按它从指纹里取
    /// （two-pass-rework/15，了结停车场 Q686——幂等那一道趁字节在手上已经给每个源页算过一份，
    /// 这里不再算第二遍；见 [`Placement::page`]）。
    fn split_and_branch(
        &self,
        index: usize,
        source: &Path,
        relative: &Path,
        bytes: Result<Vec<u8>>,
    ) -> Result<Vec<OutputPage>> {
        let read = bytes.and_then(|bytes| {
            cost::stage(cost::Stage::Decode, || self.counters.decoder.decode(&bytes))
                .with_context(|| format!("解 {} 这一页", source.display()))
        });
        let (decoded, salvage) = match read {
            Ok(decoded) => (decoded.image, decoded.salvage),
            // 一张坏图不毁掉整卷（spec 的 story 24）：记下原因就走，
            // 第二遍拿卷内统一尺寸给它留一张白页，整卷进隔离目录。
            //
            // 失败页**恒产出一张**占位页：没有像素可切，切不出第二张来。
            Err(error) => {
                let placement = Placement::new(
                    relative,
                    0,
                    OUTPUTS_PER_FAILED_PAGE,
                    self.fingerprint,
                    index,
                );
                return Ok(vec![placement.into_page(
                    source,
                    Outcome::Failed {
                        reason: format!("{error:#}"),
                    },
                )]);
            }
        };
        let color = cost::stage(cost::Stage::Identify, || color::identify(&decoded));
        let panel = self.request.profile.panel();
        if panel.color && color.is_color() {
            let image = cost::stage(cost::Stage::ToColor, || color::to_color(&decoded));
            self.color_pages(index, source, relative, image, color, salvage)
        } else {
            let image = cost::stage(cost::Stage::ToGray, || gray::to_gray(&decoded));
            self.gray_pages(index, source, relative, image, color, salvage)
        }
    }

    /// 灰度路径上一张源页产出的那几张输出页。
    ///
    /// 次序是**裁边 → 判跨页 → 拆分 → 每半再裁**（`crate::spread` 的模块文档）：
    /// 先裁再判，因为白边过宽的单页在裁之前宽高比会像跨页；每半再裁，因为装订沟那一侧的
    /// 白边是切开之后才露出来的。三段窗口叠成源页上的一块，报告只印那一个
    /// （见 [`Crop::then`]）。
    ///
    /// **没切开的那一支一个像素都不多搬**：整页那一张原样往下走，既不复制一遍，
    /// 也不白裁第二遍——一对一那条老路因此与本票落地之前逐字节相同。
    fn gray_pages(
        &self,
        index: usize,
        source: &Path,
        relative: &Path,
        image: GrayImage,
        color: PageColor,
        salvage: Option<Salvage>,
    ) -> Result<Vec<OutputPage>> {
        let request = self.request;
        let panel = request.profile.panel().resolution;
        let (crop, image) = cost::stage(cost::Stage::Crop, || {
            let crop = Crop::of_gray(&image, request.crop, salvage);
            let image = crop.apply_gray(image);
            (crop, image)
        });
        let split = cost::stage(cost::Stage::Split, || {
            Split::of_gray(&image, panel, request.split, salvage)
        });
        let pieces: Vec<(GrayImage, Piece)> = match split.halves() {
            None => vec![(image, Piece::whole(crop, split))],
            Some(halves) => halves
                .iter()
                .map(|half| {
                    let piece = cost::stage(cost::Stage::Split, || half.window().take_gray(&image));
                    cost::stage(cost::Stage::Crop, || {
                        let inner = Crop::of_gray(&piece, request.crop, salvage);
                        (inner.apply_gray(piece), Piece::half(crop, *half, inner))
                    })
                })
                .collect(),
        };
        let count = pieces.len();
        pieces
            .into_iter()
            .enumerate()
            .map(|(ordinal, (image, piece))| {
                self.gray_page(
                    source,
                    Placement::new(relative, ordinal, count, self.fingerprint, index),
                    image,
                    piece,
                    color,
                    salvage,
                )
            })
            .collect()
    }

    /// 彩色分支上一张源页产出的那几张输出页。次序与灰度那一侧逐字相同，
    /// 见 [`gray_pages`](Self::gray_pages)。
    ///
    /// 两条路各写一遍而不是收成一个泛型：收起来要给「一张页的像素」立一个 trait，
    /// 而两条路真正共用的只有那五行次序——次序本身的出处在 `crate::spread` 的模块文档里，
    /// 那是文字，不是代码。多一层抽象换回来的是同一句话说三遍。
    /// 裁边那一侧早已是这个形状（`Crop::of_gray` 与 `Crop::of_color` 两支）。
    fn color_pages(
        &self,
        index: usize,
        source: &Path,
        relative: &Path,
        image: ColorImage,
        color: PageColor,
        salvage: Option<Salvage>,
    ) -> Result<Vec<OutputPage>> {
        let request = self.request;
        let panel = request.profile.panel().resolution;
        let (crop, image) = cost::stage(cost::Stage::Crop, || {
            let crop = Crop::of_color(&image, request.crop, salvage);
            let image = crop.apply_color(image);
            (crop, image)
        });
        let split = cost::stage(cost::Stage::Split, || {
            Split::of_color(&image, panel, request.split, salvage)
        });
        let pieces: Vec<(ColorImage, Piece)> = match split.halves() {
            None => vec![(image, Piece::whole(crop, split))],
            Some(halves) => halves
                .iter()
                .map(|half| {
                    let piece =
                        cost::stage(cost::Stage::Split, || half.window().take_color(&image));
                    cost::stage(cost::Stage::Crop, || {
                        let inner = Crop::of_color(&piece, request.crop, salvage);
                        (inner.apply_color(piece), Piece::half(crop, *half, inner))
                    })
                })
                .collect(),
        };
        let count = pieces.len();
        pieces
            .into_iter()
            .enumerate()
            .map(|(ordinal, (image, piece))| {
                self.color_page(
                    source,
                    Placement::new(relative, ordinal, count, self.fingerprint, index),
                    &image,
                    piece,
                    color,
                    salvage,
                )
            })
            .collect()
    }

    /// 彩色分支上的一张：几何 → 缩放 → 编码，不进缓存、不求判据（ADR 0005 决定第 4 条）。
    ///
    /// **试算只走几何**：编码是缩放结果唯一的消费者（见 `resample::Resampler::resize_color`），
    /// 编出来的字节没人要时，缩放跟着不做（05 号票）。
    ///
    /// 进来的 `image` 已经裁过、可能切过（见 [`color_pages`](Self::color_pages)），
    /// `crop` 是那几段窗口叠起来的**源页上的一块**，报告印的就是它。
    fn color_page(
        &self,
        source: &Path,
        placement: Placement,
        image: &ColorImage,
        piece: Piece,
        color: PageColor,
        salvage: Option<Salvage>,
    ) -> Result<OutputPage> {
        let request = self.request;
        // 兜底上界在 `FitMode::target` 里，两条分支因此共用同一道（07 号票）：
        // 彩色分支上一个目标像素更贵，越界的页在这条路上先撑不住。
        let fit = request
            .fit
            .target(image.size(), request.profile.panel().resolution);
        let size = fit.size();
        // 省下的是那一整趟三平面的预缩加卷积，而报告里那一格一个字不少：`Scaling::plan`
        // 只拿源尺寸与目标尺寸做算术，`resize_color` 自己报的也正是它。
        //
        // 灰度路径上没有这一条：那边缩放结果还有判据这个消费者，而试算存在的理由
        // 正是预告那个判定。
        let (scaling, encoded) = match request.mode {
            Mode::Process => {
                let (scaled, scaling) = cost::stage(cost::Stage::Resize, || {
                    self.counters
                        .resampler
                        .resize_color(image, size, request.filter)
                })?;
                // 指纹与来路两样一起在、一起不在（见 [`Placement::new`]）：
                // `zip` 把那件事写成一句，而不是在这里再判一次。
                let record =
                    self.fingerprint
                        .zip(placement.origin.as_ref())
                        .map(|(fingerprint, origin)| {
                            Record::color(fingerprint, origin, Some(placement.page), salvage)
                        });
                let encoded = cost::stage(cost::Stage::Encode, || {
                    encode::color_png(&scaled, record.as_ref())
                })
                .with_context(|| format!("编 {} 这一页", source.display()))?;
                (scaling, Some(encoded))
            }
            Mode::DryRun => (Scaling::plan(image.size(), size), None),
        };
        Ok(placement.into_page(
            source,
            Outcome::Processed {
                size,
                crop: piece.crop,
                backstopped: fit.backstopped(),
                cut: piece.cut,
                spread_candidate: piece.candidate,
                scaling,
                color,
                branch: Branch::Color { encoded },
                salvage,
            },
        ))
    }

    /// 灰度路径上的一张：几何与几何门 → 缩放 → 判据曲线 → 进缓存。
    /// 进来的东西同 [`color_page`](Self::color_page)。
    ///
    /// **那一格装参照还是装编好的字节，由 [`Settles`] 一处说了算。**顶死的那一趟
    /// 判定在碰卷之前就定死，量化与编码当场做完（06 号票）；另外两条路存的是参照。
    fn gray_page(
        &self,
        source: &Path,
        placement: Placement,
        image: GrayImage,
        piece: Piece,
        color: PageColor,
        salvage: Option<Salvage>,
    ) -> Result<OutputPage> {
        let request = self.request;
        let panel = request.profile.panel();
        let fit = request.fit.target(image.size(), panel.resolution);
        let size = fit.size();
        // 门在这里判，也只在这里判：这一页的候选集当场定下，判据只在那一套上求。
        // 门只决定这一页——同一卷里贴住面板的页照旧拿得到抖动那一维（ADR 0007 决定第 1 条）。
        let gate = GeometryGate::of(size, panel.resolution);
        let allowed = self
            .candidates
            .for_gate(gate, image.size(), panel.resolution)
            .with_context(|| format!("{} 这一页关上了几何门", source.display()))?;
        let (scaled, scaling) = cost::stage(cost::Stage::Resize, || {
            self.counters.resampler.resize(&image, size, request.filter)
        })?;
        // 纸白对齐落在这里，**缩放之后、构造参照之前**（纸白对齐批 01 号票）：
        // 参照与其后一切量化用的都是对齐过的像素，判据两侧因此同源，
        // 量化仍然是唯一被隔离出来的变量（ADR 0002 决定第 1 条）。
        //
        // **不要把它读成「对齐过的图就是进缓存的那一份」**：另外两条路上那一格装的是编好的
        // 字节——默认那条路与顶死的那一趟一页判完当场就编（见 [`Settles`]）。
        // 对齐在两副之前，因此两副都吃得到。
        //
        // 上限取 0 时它连纸白都不量——量了也没有一页满足得了条件。
        // **默认值从 05 号票起是 4**（默认开着），走到这里的绝大多数页因此是真去量的。
        let (scaled, alignment) = white::align_white(scaled, request.white_align_limit);
        // **试算把守卫另判一遍**（纸白对齐批 02 号票第 3 条）。逐页那一层是给**点名关掉、
        // 又想知道抬上去会钳掉多少**的用户看的——他上限就是 0，上面那道短路让他每一页
        // 都读到「没开」，一个数都拿不到，票面那句话就成了只在 `--white-align-limit 255`
        // 这个他不会想到去传的咒语下才成立。
        //
        // `judge` 只判不改（三条守卫在它那一处），像素一个都不碰：这里判的正是
        // `align_white` 短路时原样交回来的那一张。判一遍的代价是每页一遍平坦掩码，
        // 摆在同一页那六档判据旁边不算什么，而 `--dry-run` 一个字节都不写。
        //
        // **照做那一趟不判**：那一趟的报告说的是「做过什么」，上限取 0 时它什么都没做，
        // 连量都不该量——短路挡的正是这份白花的工夫。
        //
        // **它不进剖面那几段**：剖面的段是照做那一趟的成本模型（见 `cost`），
        // 而这一笔只在试算上花，记进任何一段都会让那一段在两种模式下不是同一个东西。
        let alignment = match alignment {
            WhiteAlignment::Off if request.mode == Mode::DryRun => {
                white::judge(&scaled, request.white_align_limit)
            }
            settled => settled,
        };
        // 建参照与六个候选合在同一格里：参照那一侧的低通、掩蔽加权与高频起伏
        // 也是判据的工夫，只是一页只算一次（见 `metric::Reference`）。摊到格外，
        // 「判据占多少」就少算了一截，而剖面存在的理由正是这个数。
        let (reference, scores) = cost::stage(cost::Stage::Metric, || {
            let reference = Reference::new(panel, scaled);
            let scores = candidate_scores(&reference, allowed);
            (reference, scores)
        });
        let slot = match self.settles {
            // 这一页的档第一遍就定得下——默认那条路上判据一出来就定了，顶死的那一趟碰卷之前
            // 就定死了——量化与编码当场做完，那一格从头装的就是编好的字节，
            // **参照一张都不进缓存**（06、12 号票；ADR 0018）。
            // 判据曲线照旧求——上面那一格一步没少，试算说得出你点的那一档判据是多少。
            Settles::OnItsOwn | Settles::UpFront(_) => {
                let verdict =
                    decide::decide(&scores, request.profile.threshold(), self.settles.pinned());
                // 定档页那一格是 `None`：两条路上都没有哪一页把整卷拉上去
                // （与 [`driver`] 对上）。
                let recorder = self
                    .fingerprint
                    .map(|fingerprint| Recorder::new(fingerprint, None));
                let bytes = gray_bytes(
                    reference.image(),
                    verdict,
                    placement.origin.as_ref(),
                    Some(placement.page),
                    salvage,
                    recorder.as_ref(),
                )
                .with_context(|| format!("编 {} 这一页", source.display()))?;
                cost::stage(cost::Stage::CacheIn, || {
                    lock(self.cache).insert_encoded(bytes, reference.image())
                })
            }
            // 上包络那条路存参照：那一档要看完整卷才定得下。
            Settles::AfterTheVolume => cost::stage(cost::Stage::CacheIn, || {
                let block = cache::compress(reference.image());
                lock(self.cache).insert(block)
            }),
        }
        .with_context(|| format!("缓存 {} 这一页", source.display()))?;
        Ok(placement.into_page(
            source,
            Outcome::Processed {
                size,
                crop: piece.crop,
                backstopped: fit.backstopped(),
                cut: piece.cut,
                spread_candidate: piece.candidate,
                scaling,
                color,
                branch: Branch::Gray {
                    scores,
                    gate,
                    slot,
                    white: alignment,
                },
                salvage,
            },
        ))
    }
}

/// 覆盖项裁到只剩一个候选的那一档，在碰卷之前答得出来吗——答得出就是 `Some`。
///
/// [`pinned`] 拿的是**其余页那一组**的候选集，而哪一组是其余页要等整卷判完门才知道
/// （一页门成立的都没有时，门不成立的那些就当其余页）。两组给出同一个答案时那一问与分组无关，
/// 碰卷之前就答得出；一组裁到只剩一个、另一组没有时它要等整卷，
/// 那一卷照旧攒整卷参照——默认那条路上也一样（见 [`Settles::for_this_run`]）。
///
/// 门不成立那一组被覆盖项裁空（`--dither fs`）时不必等：撞上门的页整趟被拒
/// （见 [`Candidates::for_gate`]），能走完的卷里其余页只可能是门成立那一组。
///
/// **答得出那一档时缓存那一趟往返整个不必走**（06 号票，见 [`Settles::UpFront`]）：
/// [`summarize_volume`] 在覆盖项在场时早早返回，每一张灰度页拿到的就是这一档
/// 加上 [`Reason::Override`]，第一遍当场量化编码即可。
///
/// **这一问对候选是被哪一道裁剪裁到只剩一个的一视同仁**——哪几道够得着哪一维、
/// 为什么每一趟顶死都点着 `--dither`，见 ADR 0005 的《覆盖顶死的那一趟不必等到第二遍》。
fn pinned_up_front(request: &Request, candidates: &Candidates) -> Option<Option<Candidate>> {
    if request.bit_depth.is_none() && request.dither.is_none() {
        return Some(None);
    }
    let only = |set: &[Candidate]| match set {
        [only] => Some(*only),
        _ => None,
    };
    let holds = only(&candidates.holds);
    match &candidates.broken {
        None => Some(holds),
        Some(broken) => (only(broken) == holds).then_some(holds),
    }
}

/// 第一遍就照它编好了字节的那份逐页判定：默认那条路上逐页各判各的（`pinned` 是 `None`），
/// 顶死的那一趟碰卷之前就定死（`pinned` 是那一档；06 号票）。
///
/// 与 [`summarize_volume`] 那一支给出的**必须逐格相同**：那边两条出口都早早返回，
/// 每一张灰度页拿到的正是同一句 [`decide::decide`]。它只拿去对账，不进报告——
/// 报告那一份仍旧由汇总一处说了算（见 [`FirstPass`]）。
fn first_pass_verdicts(
    pages: &[OutputPage],
    threshold: Threshold,
    pinned: Option<Candidate>,
) -> Vec<Option<Verdict>> {
    pages
        .iter()
        .map(|page| {
            page.scores()
                .map(|scores| decide::decide(scores, threshold, pinned))
        })
        .collect()
}

/// 一张输出页在**源页上是哪一块**：留下的那个窗口，加上拆分那两级各自的结果。
///
/// 三项绑成一个类型而不是各占一个参数：它们由同一段（裁边 → 判跨页 → 拆分 → 每半再裁）
/// 一起算出，一起传下去，一起落进报告。摊成三个参数之后，两个可空的同型参数换了位置
/// 编译器一句话都不会说，而报告里会静默地把一张页说成另一张的形状。
#[derive(Debug, Clone, Copy)]
struct Piece {
    /// 这一张在源页上留下的那一块——裁边、切开、每半再裁三段窗口叠起来的结果
    /// （见 [`Crop::then`]）。
    crop: Crop,
    /// 这一张是那一刀的产物吗；是的话，切在哪条装订沟上、是哪一侧。整页出的是 `None`。
    cut: Option<spread::Cut>,
    /// 这一张所属的源页够得上**跨页候选**吗（拆分两级判定的第一级）。
    candidate: bool,
}

impl Piece {
    /// 没切开的那一张：整页就是一块。它仍然可能是候选——**候选而没切开就是连续跨页**。
    fn whole(crop: Crop, split: Split) -> Self {
        Self {
            crop,
            cut: None,
            candidate: split.candidate(),
        }
    }

    /// 切出来的一半：外层裁边、这一刀、每半再裁，三段窗口叠成源页上的一块。
    /// 切得开的必然是候选。
    fn half(outer: Crop, half: spread::Half, inner: Crop) -> Self {
        Self {
            crop: outer.then(half.window()).then(inner),
            cut: Some(half.cut()),
            candidate: true,
        }
    }
}

/// 一张输出页在输出容器里的位置与它的**来路**。
///
/// 两者由同一组 (源成员, 第几张, 共几张) 算出，因此一同算出、一同传下去：
/// 分开算就是两个出处，而两处一旦对不上，幂等去找的名字与真写出的名字就错开了
/// ——报告照出，输出里却少了成员（页几何批 04 号票）。
struct Placement {
    /// 它在输出容器里的相对位置（见 [`output_name`]）。
    target: PathBuf,
    /// 它的来路，写进 tEXt（见 [`Origin`]）。**只有记着的那一趟才有**，
    /// 见 [`OutputPage::origin`]。
    origin: Option<Origin>,
    /// 它来自卷里第几个源页。盖记录时页级那一份源哈希按它从指纹里取（two-pass-rework/15，
    /// 见 `metadata::Fingerprint::source_item`）——卷级那条路上取不出来，`--no-metadata` 没有指纹，
    /// 序号本身两处都无害。一个源页切出的几张共用同一个序号，它们的页级源哈希因此也共用同一份
    /// （它算的是源成员，见 [`PageSource`]）。
    ///
    /// 它**不进** [`OutputPage`]：唯一的读者是第一遍盖记录的那一下（[`Compute::gray_page`]
    /// 与 [`Compute::color_page`]），第二遍盖记录的两种页——上包络那条路上的灰度页、
    /// 失败页——按规矩都不写页级那一份。
    page: usize,
}

impl Placement {
    /// 位置总要算，来路**只在有人会读它的那一趟才造**（07 号票）。
    ///
    /// 谓词就是[指纹](Fingerprint)本身：来路唯一的消费者是 [`Recorder`]，而 [`Recorder`]
    /// 要一份指纹才在（见 `crate::process_volume` 与 [`Compute::gray_page`]）。
    /// 来路与指纹在场与否因此**恒相同**，出自这一句、没有第二处判据可以与它对不上。
    ///
    /// **这一句只说到指纹为止，反向不成立**：指纹在不等于 [`Recorder`] 在。试算那一趟
    /// 指纹照算（幂等那一道要问它），而第二遍不走、第一遍也不编——来路于是照造，没有读者。
    /// 那一处白造本票没收，记在停车场 `Q490`。
    ///
    /// `page` 是这一张来自的源页序号（见 [`Placement::page`]）。
    fn new(
        relative: &Path,
        ordinal: usize,
        count: usize,
        records: Option<&Fingerprint>,
        page: usize,
    ) -> Self {
        Self {
            target: output_name(relative, ordinal, count),
            origin: records
                .is_some()
                .then(|| Origin::new(relative, ordinal, count)),
            page,
        }
    }

    /// 配上这一张的结局，就是第一遍产出的一张输出页。源页序号到此为止，理由见
    /// [`Placement::page`]。
    fn into_page(self, source: &Path, outcome: Outcome) -> OutputPage {
        OutputPage {
            source: source.to_path_buf(),
            target: self.target,
            origin: self.origin,
            outcome,
        }
    }
}

/// 第二遍按阅读顺序要写的一格：这一趟**重做**的一张，或上一趟写的、**留下**的一张
/// （two-pass-rework/14；`CONTEXT.md` 的《留下的页》）。
///
/// 两种绑成一个枚举而不是两张表：归档卷的成员按写入顺序排，留下的与重做的**交错**着
/// 才是阅读顺序（理由与彩页为什么不在第一遍写出是同一条，见 [`second_pass`]）。
/// 卷内统一尺寸、撞名校验也走这一串——两件事问的都是整本书，不是这一趟碰过的那几张。
enum Slot<'a> {
    /// 这一趟重做的一张，连同汇总给它的判定（彩页与失败页没有）。
    Redone {
        page: &'a OutputPage,
        verdict: Option<Verdict>,
    },
    /// 上一趟写的、留下的一张，第二遍从上一趟的输出里搬过来（见 [`second_pass`]）。
    Retained(&'a RetainedPage),
}

impl<'a> Slot<'a> {
    /// 它来自哪个源页。
    fn source(&self) -> &Path {
        match self {
            Slot::Redone { page, .. } => &page.source,
            Slot::Retained(page) => &page.source,
        }
    }

    /// 它在输出容器里的相对位置。
    fn target(&self) -> &Path {
        match self {
            Slot::Redone { page, .. } => &page.target,
            Slot::Retained(page) => &page.target,
        }
    }

    /// 它写出去的像素尺寸。这一趟重做的失败页没有——它的尺寸正要由别的页定出来。
    fn size(&self) -> Option<Size> {
        match self {
            Slot::Redone { page, .. } => page.size(),
            Slot::Retained(page) => Some(page.size),
        }
    }

    /// 这一格要写出去的东西：重做的一张现取字节（编或从缓存取回），留下的一张只记下名字，
    /// 字节等写出时从上一趟的输出里搬——那一份读起来要 `&mut`，不进并行那一段。
    fn ready(&self, encode: &Encode) -> Result<Ready<'a>> {
        match *self {
            Slot::Redone { page, verdict } => Ok(Ready::Encoded {
                target: &page.target,
                bytes: encode.page(page, verdict)?,
            }),
            Slot::Retained(page) => Ok(Ready::Retained(&page.target)),
        }
    }
}

/// [`Slot`] 在第二遍上备好、轮到它写出时手上的东西。
enum Ready<'a> {
    /// 这一趟编好的字节，写到 `target`。
    Encoded {
        target: &'a Path,
        bytes: Cow<'a, [u8]>,
    },
    /// 留下的一张，轮到它时从上一趟的输出里搬（见 [`second_pass`]）。
    Retained(&'a Path),
}

/// 留下的与重做的按**阅读顺序**交错成第二遍要写的那一串（two-pass-rework/14）。
///
/// 走的是源页序：留下的那一族按记录里的次序摆，重做的那几张从第一遍的产出里按序取——
/// 第一遍产出本来就按源页序摆、同一源页切出的几张挨着（见 [`first_pass`]），
/// 因此「属于这一源页的那几张」就是产出里接下来来路相同的那一段。`verdicts` 与 `scored`
/// 等长同序（见 [`summarize_volume`]），重做的每一张把自己那一份带上。
///
/// 产出里有一张排不进去（来路对不上任何一个要重做的源页）是管线内部对不上了，回 `Err`
/// 而不是静默少写一页：少一页的卷在阅读器里与一本正经的书没有分别。
fn in_reading_order<'a>(
    volume: &Volume,
    retained: &'a Retained,
    scored: &'a [OutputPage],
    verdicts: &[Option<Verdict>],
) -> Result<Vec<Slot<'a>>> {
    let mut slots = Vec::with_capacity(scored.len() + retained.pages());
    let mut next = 0;
    for (page, kept) in volume.pages.iter().zip(retained.families()) {
        match kept {
            Some(family) => slots.extend(family.iter().map(Slot::Retained)),
            None => {
                let source = volume.identity(page);
                while let Some(redone) = scored.get(next).filter(|redone| redone.source == source) {
                    slots.push(Slot::Redone {
                        page: redone,
                        verdict: verdicts[next],
                    });
                    next += 1;
                }
            }
        }
    }
    ensure!(
        next == scored.len(),
        "第一遍产出 {} 张，只有 {next} 张排得进阅读顺序：{} 这一张来路对不上任何一个源页",
        scored.len(),
        scored[next].source.display()
    );
    Ok(slots)
}

/// 第二遍：从缓存把每一页取回来写出去。不再碰源页（ADR 0005）。
///
/// **取回来的是什么，看这一卷走的哪条路**（12 号票）。默认那条路上灰度页第一遍就编好了
/// ——一页判完的那一刻就量化、编码，缓存那一格从头装的就是编好的字节（见 [`Settles`]），
/// 这一遍于是退化成把字节按阅读顺序写出去；顶死的那一趟同样。上包络那条路上缓存里
/// 是参照，量化与编码留在这一遍：那一档要看完整卷才定得下。
/// 彩页两条路上都取第一遍编好的字节，失败页两条路上都在这里现画一张白页。
///
/// **写出按阅读顺序**，彩页也在这一遍落位。ADR 0005 决定第 4 条原话是「第一遍即写出」，
/// 那一句管的是彩页**离开灰度管线的时刻**——不进缓存、不求判据、不进上包络，这三条这里都成立。
/// 写出的时刻另有一条约束压着它：归档卷的成员按写入顺序排，而页名的字典序与阅读顺序
/// 本来就对不上（`1.png` `2.png` `10.png`）。彩页在第一遍就写进归档，混排卷的成员顺序
/// 就变成「先全部彩页、再全部灰度页」，按归档顺序翻页的阅读器会跳着读。
/// 代价认下：编好的字节要在内存里等到这一遍，且不受 `--cache-budget` 约束
/// （详见 ADR 0010）——那是编码后的 PNG，比参照小。
///
/// **失败页也在这一遍占住自己那一格**（12 号票）：一张 `uniform` 尺寸的纸白页。
/// 少写一页会让页序错位、页数对不上，而那正是「一张坏图毁掉整卷」的另一种形态。
///
/// 这一遍出的错仍然是卷级的错，不再变成失败页：它们不是坏图，是磁盘、内存与输出容器出了事，
/// 换一页重试也躲不过去。
/// **量化与编码满核跑，写出仍按阅读顺序**（13 号票）。两件事之所以分得开：编一页是纯计算、
/// 每页各编各的，而写出有次序——归档卷的成员按写入顺序排，乱一位就得让阅读器跳着读
/// （理由与彩页为什么不在第一遍写出是同一条）。
///
/// 一批编完再写一批，批量取核数：编好的字节要等到轮到它才写得出去，这一批就是它们在内存里
/// 排队的长度，因此**有界**——一页 PNG 中位不到 1 MB（measurements 的《B 类位深实测》），
/// 满核也就十几 MB。不分批而是一口气全编，那一摊就随卷长，正是有界通道要拦的东西。
/// 逐页那条路上这一批只是从缓存里取字节，分批那一层照旧——它拦的是「在内存里排队的有多少」，
/// 与那几页是现编的还是取回来的无关。
///
/// 走的是**输出页**那个序列（页几何批 03 号票）：一步一张输出页，成员名跟着页走
/// （[`OutputPage::target`]），不由这一遍按源页序数出来——一个源页产出几张时，
/// 数出来的那个序号会静默地把另一张的字节写到这一张的位置上。
///
/// **中止让它半路回来**（ADR 0013 决定第 2 条）：写出那一层的循环头上问一次闩，
/// 答中止就当场 `Ok(())`。它不必把这件事写进返回值——闩只升不降，调用方再问一次
/// 恒得同一个答案（见 [`progress::Events::aborting`]），而那里正是决定收不收尾的地方。
///
/// **留下的页原样搬**（two-pass-rework/14、15）：从上一趟的输出（`prior_output`，比对那一步开着的
/// 同一份）整页读回，一个字节不改，照写页那条路写进这一趟的容器——不解码、不判、不编。
/// 记录里没有要改写的东西：页级那条路上每一页的记录只取决于它自己（收掉卷级那一项之后，
/// 停车场 Q681 里的另一条路成了唯一的路），产物因此与整卷重做的逐字节相同。
fn second_pass(
    slots: &[Slot],
    encode: &Encode,
    sink: &mut Sink,
    mut prior_output: Option<&mut sink::Written>,
    events: progress::Events,
) -> Result<()> {
    for batch in slots.chunks(cores()) {
        let ready: Vec<Ready<'_>> = batch
            .par_iter()
            .map(|slot| slot.ready(encode))
            .collect::<Result<Vec<_>>>()?;
        for page in ready {
            // **页边界那个检查点**（ADR 0013 决定第 2 条），而且是三段里唯一一个此刻
            // 真有东西可丢的：写进去的页都在那格 `partial` 里，不收尾就整格丢掉。
            // 停在写出这一侧而不是编码那一侧：白编一批（至多核数张）远比多写一页便宜，
            // 而「已经写了几页」才是中止要回答的那个问题。
            if events.aborting() {
                return Ok(());
            }
            cost::stage(cost::Stage::Write, || match page {
                Ready::Encoded { target, bytes } => sink.write_page(target, &bytes),
                // 留下的页从上一趟的输出里原样搬过来（two-pass-rework/14）。那一份在按页那一支上
                // 恒打开着（见 [`Reuse::ByPage`]）；不在就是调用方拿错了路，当场报。
                Ready::Retained(target) => match prior_output.as_mut() {
                    Some(output) => sink.write_page(target, &output.bytes_of(target)?),
                    None => bail!("留下 {} 这一页时没有上一趟的输出可搬", target.display()),
                },
            })?;
            events.step();
        }
    }
    Ok(())
}

/// 第二遍上每条计算线程共用的那一摊，与第一遍的 [`Compute`] 同一个用意。
struct Encode<'a> {
    /// 失败页按它出（12 号票的卷内统一尺寸）。
    uniform: Size,
    cache: &'a Mutex<cache::PageCache>,
    recorder: Option<&'a Recorder<'a>>,
}

impl Encode<'_> {
    /// 一页写出去的那串字节。三种页各有各的来路，但出来的都是一页 PNG。
    ///
    /// 出的是 [`Cow`]：彩页的字节第一遍就编好了，这里**借**它而不是复制一份。
    /// 那一摊本来就不受 `--cache-budget` 约束（ADR 0010），再翻一倍不合适。
    fn page<'p>(&self, page: &'p OutputPage, verdict: Option<Verdict>) -> Result<Cow<'p, [u8]>> {
        let Self {
            uniform,
            cache,
            recorder,
        } = *self;
        let source = page.source.display();
        match &page.outcome {
            Outcome::Failed { .. } => {
                // 占位页按 1bit 编，不跟卷级基准档走。它不是一个**判定**——它没进过候选集、
                // 没求过判据，卷级那一档说的是「这一卷的内容要几档灰」，而这一页没有内容。
                // 位深是编码属性（`CONTEXT.md`），而整页只有一个取值时 1bit 恰好装得下它；
                // 换个更宽的档也写不出别的字节，编码器那一层照旧会挑最窄的（ADR 0004）。
                let record = recorder
                    .zip(page.origin.as_ref())
                    .map(|(recorder, origin)| recorder.failed(origin));
                cost::stage(cost::Stage::Encode, || {
                    encode::png(&placeholder(uniform), BitDepth::One, record.as_ref())
                })
                .map(Cow::Owned)
                .with_context(|| format!("编 {source} 这一页的占位页"))
            }
            Outcome::Processed {
                branch: Branch::Color { encoded },
                ..
            } => Ok(Cow::Borrowed(
                encoded.as_deref().expect("照做的那一遍第一遍就编过彩页"),
            )),
            Outcome::Processed {
                branch: Branch::Gray { slot, .. },
                salvage,
                ..
            } => {
                // 取页要动缓存那本账，因此在锁里；量化与编码在锁外——贵的是后两件。
                let held = cost::stage(cost::Stage::CacheOut, || lock(cache).take(*slot))
                    .with_context(|| format!("从缓存取 {source} 这一页"))?;
                // 默认那条路与顶死的那一趟这一页第一遍就编好了（12、06 号票），这一遍只把它写出去。
                let reference = match held {
                    cache::Held::Encoded(bytes) => return Ok(Cow::Owned(bytes)),
                    cache::Held::Reference(reference) => reference,
                };
                let verdict = verdict.expect("灰度路径上必有判定");
                // 参照留到这一遍才编的，只有上包络那条路（与 Q635 那一角）：这一页的档由全卷定，
                // 那是卷级那条路，页级那一份不写（源页序号不给，见 `metadata::Fingerprint::source_item`）。
                gray_bytes(
                    &reference,
                    verdict,
                    page.origin.as_ref(),
                    None,
                    *salvage,
                    recorder,
                )
                .map(Cow::Owned)
                .with_context(|| format!("编 {source} 这一页"))
            }
        }
    }
}

/// 一张灰度页从参照走到写得出去的那串字节：量化 → 盖记录 → 编码。
///
/// **两遍共用这一处。** 默认那条路与顶死的那一趟 [`Compute::gray_page`] 在第一遍调它
/// （12、06 号票），上包络那条路上 [`Encode::page`] 在第二遍调它。写出的字节因此不因为
/// 在哪一遍编的而不同——「输出字节与本票之前逐字节相同」靠的正是这一句只有一处。
///
/// 掐表也在这里：`Quantize` 与 `Encode` 两格因此两条路上量的是同一件事。
fn gray_bytes(
    reference: &GrayImage,
    verdict: Verdict,
    origin: Option<&Origin>,
    page: Option<usize>,
    salvage: Option<Salvage>,
    recorder: Option<&Recorder>,
) -> Result<Vec<u8>> {
    let quantized = cost::stage(cost::Stage::Quantize, || {
        quantize::quantize(reference, verdict.candidate)
    });
    // 两个调用处传进来的记录器与来路**恒是一起在、一起不在**：两处的记录器都由同一份指纹派生
    // （[`Encode`] 那一份在 `crate::process_volume`，第一遍那一份在 [`Compute::gray_page`]），
    // 而来路的在场与否问的正是那份指纹（见 [`Placement::new`]）。`zip` 因此不是在防一个
    // 真会发生的组合，是把那句话写成编译器认得的形状。
    // 源页序号不在这个 `zip` 里：它是真的可空——第一遍那一处有，第二遍那一处没有
    // （见 [`Placement::page`]）。
    let record = recorder
        .zip(origin)
        .map(|(recorder, origin)| recorder.gray(origin, page, verdict, salvage));
    cost::stage(cost::Stage::Encode, || {
        encode::png(&quantized, verdict.candidate.bit_depth, record.as_ref())
    })
}

/// 本次调用在这一卷上的幂等依据（ADR 0006：同一批 tEXt 字段兼作幂等依据）。
///
/// 源哈希在这里算，**按这一趟的作用域算一种**（two-pass-rework/15；`CONTEXT.md` 的《源哈希》）：
/// `by_page` 答页级——每个源页各一份、每个透传文件各一份，页的那一份进记录，透传文件的那一份
/// 只拿去比；答卷级——页与透传文件按次序喂进同一个累加器，收口成全卷一个数。
/// 哪一种由 [`Settles`] 定，这里只照办。
///
/// 这一遍**把源字节多读一遍**——它在第一遍解码之前，而第一遍还要再读一次。这笔成本换不掉：
/// 彩页在第一遍就编好并写进 tEXt（ADR 0010），那一刻这一页记录里源那一项必须已经齐了。
/// 换来的是命中时一趟都不用做——多读一遍字节，省掉的是整卷的解码、缩放、判据与编码。
/// `--no-metadata` 连这一遍都不读：那时既没有记录可写，也没有依据可比。
///
/// 它走的是与第一遍同一个[读取层](read)，因此在没有寻道惩罚的盘上这一遍也是并发读的。
/// 喂哈希那一端仍**严格按成员次序**——卷级源哈希是有序的，乱一位整卷的指纹就变了，
/// 页级那一份按成员序号落位，而读取层交付本来就有序（见 `read` 的模块头）。
///
/// **[读取端是一个归档句柄](source::ReadingEnd::Archive)的卷上它也并发**，与第一遍不同：
/// 并发度取的是 [`IoPlan::fingerprint`] 那一格，
/// 不是 [`IoPlan::readers`]（为什么两路各有一个数，见 [`IoPlan`] 的《为什么是两路》）。
/// 并行的是**解**，不是**喂**：几条读取线程各拿一个自己的归档句柄各解各的成员，
/// 交付仍按成员序号，因此同一卷串行与并行两趟的指纹**逐字节相同**。
fn volume_fingerprint(
    volume: &mut Volume,
    request: &Request,
    io: &IoPlan,
    by_page: bool,
    events: progress::Events,
) -> Result<Fingerprint> {
    let Volume {
        pages,
        extras,
        reader,
        ..
    } = volume;
    let source_pages = pages.len();
    let members: Vec<&Member> = pages.iter().chain(extras.iter()).collect();
    // 两种作用域各一个累加器，一趟只开一个：算出来没人读的那一份一个都不算（story 30）。
    let mut feeding = if by_page {
        Feeding::Page(PageSources::with_capacity(source_pages, extras.len()))
    } else {
        Feeding::Volume(Box::new(metadata::SourceHasher::new()))
    };
    for read in read::reads(reader, &members, io.fingerprint.count, read::BUDGET) {
        // **页边界那个检查点**（ADR 0013 决定第 2 条）：中止停在成员边界上。
        // 并发之下这一条不变：交付按成员序号，`break` 因此停在一个真正的成员边界上；
        // 走出去的这个循环把 `Reads` 丢掉，几条读取线程当场收摊（见 `read::Throttle::stop`）。
        // 抢跑解出来的那几个成员的字节跟着一起丢掉——停的是**喂**，解到哪儿不影响这份哈希。
        // 喂了一半的哈希不是这一卷的指纹，[`process_volume`] 不拿它去问幂等，
        // 也不让它走出那一卷（见那里的《中止：回 `None`》）。
        if events.aborting() {
            break;
        }
        let relative = &members[read.index].relative;
        match &mut feeding {
            // 读不出字节的成员在这一遍不算失败：它在第一遍里才变成失败页（12 号票），
            // 而这一遍排在第一遍之前。这里把它记成「读不出来」照样喂进哈希——
            // 拦在这里，一个坏成员就会毁掉整卷，正是本票要拆掉的那件事。
            Feeding::Volume(hasher) => match &read.bytes {
                Ok(bytes) => cost::stage(cost::Stage::Hash, || hasher.member(relative, bytes)),
                Err(_) => hasher.unreadable(relative),
            },
            // 读不出字节的成员在第一遍里会变成失败页，而失败页不写页级依据
            // （见 [`Recorder::failed`]）——它在这里也就没有可比的，记成「没有」。
            Feeding::Page(sources) => {
                let hashed = read.bytes.as_ref().ok().map(|bytes| {
                    cost::stage(cost::Stage::Hash, || PageSource::of(relative, bytes))
                });
                if read.index < source_pages {
                    sources.push_page(hashed);
                } else {
                    sources.push_extra(hashed);
                }
            }
        }
        events.step();
    }
    let source = match feeding {
        Feeding::Volume(hasher) => SourceHash::Volume(hasher.finish()),
        Feeding::Page(sources) => SourceHash::Page(sources),
    };
    Ok(Fingerprint::new(request, source))
}

/// 幂等那一道正在喂的那个累加器：两种作用域各一种（见 [`volume_fingerprint`]）。
///
/// 卷级那一支装箱：blake3 的累加器自带近 2 KB 的状态，而这个枚举一卷只有一个，
/// 装箱只为让两支一样大（clippy 的 `large_enum_variant`），不为省什么。
enum Feeding {
    /// 卷级：页与透传文件按次序喂进同一个累加器，收口成全卷一个数。
    Volume(Box<metadata::SourceHasher>),
    /// 页级：每个成员各算一份，页与透传文件各归各的表。
    Page(PageSources),
}

/// 上一趟写在**干净去处**的输出**能复用多少**——幂等那一道比出来的答案（`CONTEXT.md` 的《幂等这一道》）。
///
/// 三种，按代价从小到大排：整卷一页不做、按页只做变了的、整卷重做。
enum Reuse {
    /// 上一趟的输出还齐着——**整卷跳过**（spec 的 story 8）。`page_count` 是上一趟写在那儿的
    /// 输出页数。
    ///
    /// 两条路各有各的「齐」（two-pass-rework/15）：卷级那条路上是每一页都记着这份指纹、
    /// 透传文件都在；页级那条路上是每一页各自都没变、透传文件各自都没变、输出里再没有别的
    /// （见 [`compare_with_the_prior_output`]）。
    Whole { page_count: usize },
    /// 页级那条路上卷不齐，**按页**（two-pass-rework/14）：逐源页答留不留（[`Retained`]）。
    /// 一页都不留也是这一支——那时与整卷重做走的是同一条路，只是留下的页由搬代替了做。
    ///
    /// `output` 是打开着的上一趟输出，留下的页从它里面原样搬（见 [`second_pass`]）。
    /// 归档那一支上它握着最终位置上那个文件的句柄，从这里一直握到第二遍搬完——
    /// 中间隔着第一遍与决策点（ADR 0012）；那段时间里 Windows 上动不了上一趟的输出，认下。
    ByPage {
        retained: Retained,
        output: sink::Written,
    },
    /// 无从比：头一趟、上一趟的输出不在，或卷级那条路上卷不齐——**整卷重做**。
    Nothing,
}

/// 这一卷逐源页「留不留」的答案（two-pass-rework/14）：按源页序，留下的那一族是它每一张的
/// [`RetainedPage`]，重做的那一页是 `None`。整卷重做就是每一格都 `None`。
struct Retained(Vec<Option<Vec<RetainedPage>>>);

impl Retained {
    /// 一页都不留：整卷重做。
    fn nothing(source_pages: usize) -> Self {
        Self((0..source_pages).map(|_| None).collect())
    }

    /// 逐源页的答案，按源页序。
    fn families(&self) -> &[Option<Vec<RetainedPage>>] {
        &self.0
    }

    /// 要重做的源页序号，按源页序。第一遍只走它们。
    fn redo(&self) -> Vec<usize> {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, kept)| kept.is_none())
            .map(|(index, _)| index)
            .collect()
    }

    /// 留下的**输出**页有几张。报告印的是它（`VolumeReport::retained_pages`）。
    fn pages(&self) -> usize {
        self.0.iter().flatten().map(Vec::len).sum()
    }
}

/// 上一趟写的还在、这一趟**留下**的一张输出页（`CONTEXT.md` 的《留下的页》）。
struct RetainedPage {
    /// 它来自哪个源页，与 [`OutputPage::source`] 同一个写法：撞名要指得出是哪两个源成员。
    source: PathBuf,
    /// 它在输出容器里的相对位置，与上一趟写它时同一个（见 [`output_name`]）。
    target: PathBuf,
    /// 上一趟写它时的像素尺寸，读自它的记录。卷内统一尺寸要数它（见 [`uniform_size`]）。
    size: Size,
}

/// 拿上一趟的输出比这一趟的依据（spec 的 story 8；two-pass-rework/14 把答案从两种扩成三种）。
///
/// `output` 是上一趟写在干净去处的输出，由调用方开好交进来（头一趟没有，那就是整卷重做）；
/// 按页那一支把它原样带出去，留下的页第二遍从它里面搬。
///
/// **逐页比的是这一趟走的那一种依据**，只此一种（two-pass-rework/15，见 [`PageRecord::matches`]）：
/// 三项没变、来路对得上，加上卷级那条路的全卷那一个数或页级那条路的这一页自己那一份。
/// 带着另一种的记录不认——旧的默认路径输出（两种都带）因此不命中，重做一次之后转成新形态。
///
/// **「齐」两条路各有各的说法**，整卷跳过只在齐的时候：
///
/// - 卷级那条路（`--envelope`，与覆盖项只点了一维而两组门混着的那一角）：每一页都记着这份
///   指纹、透传文件都在——与 two-pass-rework/15 之前逐字相同。不齐就整卷重做：那条路上
///   一页的档由全卷定，没有第二问。透传文件变了、源里删了一页，全卷那一个数都看得见。
/// - 页级那条路（默认）：每一页各自都没变、每个透传文件各自都没变（输出里那一份重新算出来
///   与源那一份相同）、**输出里再没有别的**（[`sink::Written::holds_nothing_but`]）。
///   后两条是卷级那一个数从前顺手盖住的：页级各比各的，看不见透传文件的内容、
///   看不见源里删掉的那一页留在输出里的陈旧产物。不齐就按页：对得上的留下，其余重做，
///   透传文件照旧整卷重写，陈旧产物收尾清掉。
///
/// **只认干净的那个去处**：隔离目录里那一份不是做完了的输出，失败清单每一趟都要重新给得出来
/// （spec 的 story 26）。上一趟进了隔离而更早一趟留在干净去处的那一份，照旧拿来比——
/// 坏页修好之后，没变的页从那一份里留下。
///
/// 一页都没有的卷永远不命中：记录随页走，没有页就没有地方放它。这一支从 ADR 0014
/// 之后**够不着了**——一页都没有的东西不是卷，预扫就把它丢掉了；留着这一句，
/// 是因为「记录随页走」这条不变量要写在它成立的地方。
///
/// # 名单从哪儿来（页几何批 04 号票）
///
/// 从前它由 `page_targets` 在碰像素之前预告出来，逐个去比。跨页拆分落地之后那条预告
/// 不成立了——一个源页产出几张由内容决定（有没有装订沟），而幂等的全部意义是在解码之前答完。
///
/// 方向因此反过来：名单从**上一趟写在输出里的记录**读回来。一张输出页自己说得出
/// 它来自哪个源成员、那一族该有几张（[`Origin`]），于是按源页逐族去探——
/// 先试一对一那个名字，不在就试切开的那一族，头一张说出总共几张，再把余下的逐个对上
/// （见 [`written_family`]）。
///
/// **「输出里少一张就该察觉」这条能力因此没有丢，而且比从前更严**：
/// 一族两张里删掉一张，剩下那一张仍写着「1/2」，第二张找不到，那一族重做
/// （`p0-hardening/03` 靠的正是这条能力）。缺了那个计数就只剩「至少有一张」，
/// 一张跨页被删掉半边会静默地留在输出里。
///
/// 命中时答的是**上一趟写在那儿的输出页数**，不是这一趟预告出来的数：
/// 那个数眼下只给得出上界（见 [`MemberCounts`]），而报告里印的那个要是真数。
///
/// # 删一页、加一页、改名（two-pass-rework/14）
///
/// 输出成员名由源成员名推出（[`output_name`]），页级依据连名字一起喂（[`PageSource`]）：
/// 删掉源里一页，它那一族在输出里成了陈旧产物，收尾整个换掉时一并清走（见 `crate::sink`），
/// 其余页各自对得上、各自留下；加一页、改名一页，新名字下没有记录，那一页重做，
/// 旧名字下的那几张同样是陈旧产物。阅读顺序里的位置**不进依据**——它由名字的次序定，
/// 挪动位置的那几页字节与名字都没变，留下它们正对。
fn compare_with_the_prior_output(
    output: Option<sink::Written>,
    volume: &Volume,
    fingerprint: &Fingerprint,
    lodgers: &sink::Lodgers,
) -> Reuse {
    let Some(mut written) = output.filter(|_| !volume.pages.is_empty()) else {
        return Reuse::Nothing;
    };
    let mut page_count = 0;
    let mut retained: Vec<Option<Vec<RetainedPage>>> = Vec::with_capacity(volume.pages.len());
    for (index, page) in volume.pages.iter().enumerate() {
        let relative = &page.relative;
        // 一族齐不齐是容器的事实（[`written_family`]），齐了之后逐张比这一趟的依据
        // （[`PageRecord::matches`]），这里只数「对得上几族」。
        let family = written_family(&mut written, relative).filter(|family| {
            family.iter().enumerate().all(|(ordinal, written)| {
                written
                    .record
                    .matches(fingerprint, index, relative, ordinal, family.len())
            })
        });
        page_count += family.as_ref().map_or(0, Vec::len);
        retained.push(family.map(|family| {
            family
                .into_iter()
                .map(|written| RetainedPage {
                    source: volume.identity(page),
                    target: written.target,
                    size: written.record.size,
                })
                .collect()
        }));
    }
    let every_page = retained.iter().all(Option::is_some);
    match fingerprint.source() {
        SourceHash::Volume(_) => {
            let extras_in_place = volume
                .extras
                .iter()
                .all(|extra| written.holds(&extra.relative));
            if every_page && extras_in_place {
                Reuse::Whole { page_count }
            } else {
                Reuse::Nothing
            }
        }
        SourceHash::Page(sources) => {
            if every_page && nothing_else_changed(&mut written, volume, sources, &retained, lodgers)
            {
                Reuse::Whole { page_count }
            } else {
                Reuse::ByPage {
                    retained: Retained(retained),
                    output: written,
                }
            }
        }
    }
}

/// 页级那条路上，每一页都留得下之后整卷跳过还差的两问（two-pass-rework/15）：透传文件各自没变、
/// 输出里再没有别的。两问都是卷级那一个数从前顺手盖住的——页级各比各的看不见它们。
///
/// 透传文件不带记录：输出里那一份读回来按同一条规矩算一份，与源那一份比。
/// 「再没有别的」问的范围与收尾清陈旧产物的同一个（[`sink::Written::holds_nothing_but`]）。
fn nothing_else_changed(
    written: &mut sink::Written,
    volume: &Volume,
    sources: &PageSources,
    retained: &[Option<Vec<RetainedPage>>],
    lodgers: &sink::Lodgers,
) -> bool {
    let extras_unchanged = volume.extras.iter().enumerate().all(|(index, extra)| {
        sources.extra(index).is_some_and(|ours| {
            written
                .bytes_of(&extra.relative)
                .is_ok_and(|bytes| PageSource::of(&extra.relative, &bytes) == *ours)
        })
    });
    let members = retained
        .iter()
        .flatten()
        .flatten()
        .map(|page| page.target.as_path())
        .chain(volume.extras.iter().map(|extra| extra.relative.as_path()));
    extras_unchanged && written.holds_nothing_but(members, lodgers)
}

/// 上一趟的输出里的一张页：它的成员名，与它记着的记录。
struct WrittenPage {
    target: PathBuf,
    record: PageRecord,
}

/// 上一趟的输出里，一个源页那一族输出页：每一张的成员名与记录，按阅读顺序；
/// 缺一张、来路对不上，就是 `None`——那一族这一趟重做。
///
/// 两支：一对一那个名字在，就只此一张（记录自己也得说是 `1/1`——名字对上而记录说
/// 「共两张」的话，另一张要么被删了、要么是别的参数跑出来的）；不在，就按切开那一族探，
/// 头一张（`…-1.png`）的记录说出总共几张，剩下的逐个对上。
///
/// 它只问**来路**，不问依据：一族齐不齐是容器的事实，齐了之后按这一趟的依据比
/// 由调用方定（见 [`compare_with_the_prior_output`]）。
///
/// 名字怎么拼只有一个出处（[`output_name`]），两支拼的都是它。
fn written_family(written: &mut sink::Written, relative: &Path) -> Option<Vec<WrittenPage>> {
    let one = output_name(relative, 0, 1);
    if let Some(record) = written.record_of(&one) {
        return record.is_the_page(relative, 0, 1).then(|| {
            vec![WrittenPage {
                target: one,
                record,
            }]
        });
    }
    // 一对一那个名字不在。那这一族要么是切开的，要么根本没写出来——头一张说了算：
    // 它记着自己那一族共几张，而余下几张的名字由那个数推得出来。
    let first_of_many = output_name(relative, 0, MORE_THAN_ONE);
    let first = written.record_of(&first_of_many)?;
    let count = first.origin.as_ref()?.count();
    if count < MORE_THAN_ONE || !first.is_the_page(relative, 0, count) {
        return None;
    }
    let mut family = vec![WrittenPage {
        target: first_of_many,
        record: first,
    }];
    for ordinal in 1..count {
        let target = output_name(relative, ordinal, count);
        let record = written.record_of(&target)?;
        if !record.is_the_page(relative, ordinal, count) {
            return None;
        }
        family.push(WrittenPage { target, record });
    }
    Some(family)
}

/// 卷级上包络的定档页序号，写进 tEXt 那句 `volume-p95, driven by page 087` 用它。
///
/// 另外三种卷级判定没有定档页可指：覆盖项顶掉了判定，默认路径上卷级那一层没开，
/// 而跳过的卷根本走不到写出这一步。
fn driver(verdict: Option<VolumeVerdict>) -> Option<usize> {
    match verdict {
        Some(VolumeVerdict::Envelope(envelope)) => Some(envelope.driver),
        _ => None,
    }
}

/// 把这一页的参照与每个候选各比一遍。
///
/// 候选先裁再求值，顺序是 ADR 0003 定的：被裁掉的候选不进入判据。
/// 这里只出量，拿量去和阈值比在 `decide`。
fn candidate_scores(reference: &Reference, allowed: &[Candidate]) -> Vec<CandidateScore> {
    allowed
        .iter()
        .map(|&candidate| CandidateScore {
            candidate,
            score: metric::score(
                reference,
                &quantize::quantize(reference.image(), candidate),
                candidate.bit_depth,
            ),
        })
        .collect()
}

/// 门是 `gate` 的页可用的候选集，由小到大。
///
/// 四道裁剪，全部发生在判据求值之前：位深按面板灰阶数裁（ADR 0003），抖动模式按几何门裁
/// （ADR 0007），`--bit-depth` 与 `--dither` 各再裁自己那一维。前两道是界，后两道是覆盖项，
/// 但作用方式是同一个——都只从候选集里拿走东西，谁都放不回被拿走的。
///
/// 裁空了就报错，而**那件事在裁之前问**（见 [`why_nothing_is_left`]）：面板显示不出来、
/// 或几何上到不了眼睛的那些候选，写出去也是白写，宁可当场拒绝也不静默照写。
/// 门那一维裁空的时候，拒绝的报出的是**哪一页**撞上的门（见 [`Candidates::for_gate`]）。
///
/// 回来的这一套因此**非空**，这里不再数一遍：候选集是两维的全积（[`Candidate::all`]），
/// 积空当且仅当有一维空，而两维各自那一问都过了。**两维本身也空不了**——位深那一维
/// 1bit 恒在里面（面板灰阶数至少 2 级，[`Profile::with_gray_levels`] 挡着），
/// 抖动那一维门的两侧都留着 `Dither::Off`（见 [`Dither::candidates`]）。
/// 数一遍就是把同一件事判第二次。
fn candidates(request: &Request, gate: GeometryGate) -> Result<Vec<Candidate>> {
    if let Some(said) = why_nothing_is_left(request, gate) {
        return Err(Refusal(said).into());
    }
    let panel = request.profile.panel();
    Ok(Candidate::all(panel.gray_levels, gate)
        .into_iter()
        .filter(|candidate| {
            request
                .bit_depth
                .is_none_or(|bit_depth| candidate.bit_depth == bit_depth)
        })
        .filter(|candidate| {
            request
                .dither
                .is_none_or(|dither| candidate.dither == dither)
        })
        .collect())
}

/// 覆盖项与面板对不对得上，在碰卷之前先问一次。
///
/// 几何门此刻还没有页可判，先当它成立：门那一侧裁空的候选集只有等到第一遍里
/// 真撞上那一页才拦得住（见 [`Candidates::for_gate`]）。
fn ensure_the_overrides_leave_a_candidate(request: &Request) -> Result<()> {
    candidates(request, GeometryGate::Holds).map(|_| ())
}

/// 覆盖项把哪一维裁空了——裁空了给出那句话，两维都过得去回 `None`。
///
/// **两维各判各的，各只有一处判定。**位深那一维问「点名的那一档，这块面板写不写得出」
/// （ADR 0003 的硬上界）；抖动那一维问「这一趟咬上互锁 ③ 了吗」，判定在
/// [`Interlock::dither_outside_the_gate`]，这里只取它的答案——那条拒绝**由互锁驱动**，
/// 不再有第二处形态拿去核对。
///
/// 次序不是随手排的：两维一起对不上时报的是位深那一句。它指得出一道**动得了**的界，
/// 而抖动那一句指的是页的几何事实。
///
/// 两道界只有一道动得了：面板灰阶数走 `--gray-levels`（ADR 0003），几何门动不了——
/// 它是页的几何事实，不是一个可以放宽的档位。
///
/// **两支说得出的话不一样全，那不是漏。**位深那一句碰卷之前就说得全——面板灰阶数是
/// **这一趟**的事实。抖动那一支回的只有[规则那一句](Interlock::DitherOutsideTheGate)：
/// 出路那一半要**这一页**才答得出（够得着以高为准的页与够不着的页听见的不是同一句，
/// 21 号票、停车场 Q102），补上它并戴上 [`Refusal`] 的是 [`Candidates::for_gate`]。
/// 门是页的几何事实，判定与措辞因此都只在碰上那一页时才收得了口。
///
/// 出来的是那句话本身，不是一个错误。**位深那一支戴 [`Refusal`] 由 [`candidates`] 做**，
/// 而抖动那一支走的是另一条路（上一段说的那件事）：它那句话要补全，戴标记因此也由
/// 补全它的 [`Candidates::for_gate`] 做。**两支仍不会一支戴一支忘**——各自那一处都只有
/// 一个出口，而 [`Candidates::new`] 那一行 `.ok()` 是它们分家的地方，写在那儿。
/// 两支都是**覆盖项**与面板对不上，错在这一趟的参数上，换一个卷不会变好（05 号票）。
fn why_nothing_is_left(request: &Request, gate: GeometryGate) -> Option<String> {
    let panel = request.profile.panel();
    let depths = BitDepth::candidates(panel.gray_levels);
    if let Some(bit_depth) = request.bit_depth.filter(|depth| !depths.contains(depth)) {
        let listed = depths
            .iter()
            .map(BitDepth::to_string)
            .collect::<Vec<_>>()
            .join("、");
        return Some(format!(
            "{bit_depth} 越过了面板的 {} 级灰阶：这块面板上写得出的是 {listed}。\
             真要写 {bit_depth}，先按实测用 --gray-levels 抬高上界",
            panel.gray_levels
        ));
    }
    // 抖动那一维：几何门不成立而 `--dither` 点了抖动。那正是互锁 ③，
    // 处置是维持拒绝（页几何批 05 号票）。**出路那一半不在这里**，见上面那一段。
    Interlock::dither_outside_the_gate(request.dither, gate)
        .then(|| Interlock::DitherOutsideTheGate.to_string())
}

/// 互锁 ③ 咬上时那条拒绝的说法（05 号票的处置 ③：**维持拒绝**）。
///
/// 规则那一句由 [`Interlock`] 自己说——同一句还要从 `--help` 里出来，措辞只有那一份。
/// 这里补的是**这一页**才知道的那件事：适配方式那一侧还有没有出路。撞上的是哪一页
/// 由错误链外层带着（见 [`Compute::gray_page`]）。
///
/// **按页分岔**（21 号票，收停车场 Q102）：`by_height_holds` 答的是
/// 「换成以高为准之后，**这一页**的门成不成立」，判定在 [`Candidates::for_gate`]。
/// 两支各只说对这一页成立的那一半——从前这句话把两条路的例外一次全说，
/// 够得着出路的人得先读一条对他不成立的建议，够不着的人得先读一条劝他敲了会撞第二次的命令。
///
/// - **门跟着成立**：`--fit height` 把这一页放大到面板高（页几何批 01 号票），
///   那条出路对**这一页**当真，例外因此不必再提。
/// - **门仍不成立**：这一页宽高比极端到以高为准算出的目标尺寸越过
///   [兜底上界](FitMode::target)、会被退回 fit-inside（07 号票）——退回来的仍是
///   一张 fit-inside 的页。劝它换 `--fit height` 是**假话**，改说剩下的那两条路。
///   这一支与这一趟点的是哪个适配方式无关：以高为准上走得到拒绝的页恒是这一种。
///
/// 出来的是那句话本身，不是一个错误，理由见 [`why_nothing_is_left`]（05 号票）。
///
/// **记号里面那个空格是[不许断的那个空格](HARD_SPACE)**：这句话劝人换一条命令，
/// 断成两行之后抄不出一条能用的命令（停车场 Q106）。规矩只有一处出处，就是那条公共 API。
fn dither_outside_the_gate_error(by_height_holds: bool) -> String {
    let way_out = if by_height_holds {
        format!("改得动的是几何：--fit{HARD_SPACE}height 把这一页放大到面板高，门跟着成立")
    } else {
        format!(
            "适配方式这一侧没有出路：以高为准让每一页都贴住面板高，而这一页算出的目标尺寸\
             越过 {} 像素、被兜底上界退回 fit-inside 出（07 号票），门是在那张退回来的页上判的",
            max_target_pixels()
        )
    };
    format!(
        "{}。{way_out}。剩下两条路——不点 --dither{HARD_SPACE}fs（判据自己会替这一页把抖动关掉），\
         或换一张宽高比没这么极端的源页",
        Interlock::DitherOutsideTheGate
    )
}

/// 一个源页**最多**产出几张输出页：跨页从装订沟上切一刀，因此是两张（页几何批 04 号票）。
///
/// 真产出几张由内容决定，解完像素才知道（见 `crate::spread`）。这个数是碰像素之前
/// 给得出来的那个**上界**：进度步数照它预告（[`volume_steps`]），第一遍走完拿它对一次区间
/// （[`process_volume`]）。
///
/// 它不是「切几刀」的配置项：切点只有一个（装订沟），两半就是两张。
const MAX_OUTPUTS_PER_SOURCE_PAGE: usize = 2;

/// 拼「切开那一族里的第一张」这个名字时传的张数。
///
/// [`output_name`] 只分「一张」与「不止一张」两种写法，具体几张不进名字——
/// `001-1.png` 无论那一族有两张还是三张都是这个名字。幂等按名字探那一族时因此随便传一个
/// ≥2 的数就够（见 [`written_family`]）；名字里那个 `-1` 不是「共两张」的意思。
const MORE_THAN_ONE: usize = 2;

/// 这一趟一个源页最多产出几张。拆分关着时恒 1——那时预告是精确的，不是上界。
fn max_outputs_per_source_page(request: &Request) -> usize {
    if request.split.on {
        MAX_OUTPUTS_PER_SOURCE_PAGE
    } else {
        1
    }
}

/// 一张失败页产出几张占位页。**恒 1，而且与 [`MAX_OUTPUTS_PER_SOURCE_PAGE`] 无关**：
/// 它没有像素可切，切不出第二张来（12 号票：失败页以卷内统一尺寸留白占位，页序不断）。
///
/// 两者已经分家：拆分让好页的 N 随内容而变（页几何批 04 号票），而解不出像素的那一页
/// 仍然只出一张——预告那个上界因此对它偏大一张，那正是[区间断言](process_volume)容得下的。
const OUTPUTS_PER_FAILED_PAGE: usize = 1;

/// 一个源页产出的第 `ordinal` 张输出页（从 0 起）在输出容器里的相对位置，
/// `count` 是这一源页总共产出几张。
///
/// 扩展名一律换成 png。**只产出一张时名字就是源页名换扩展名**——一对一那条老路
/// 一个字符都不多，升级的人手上的输出因此不会有成员被改名。产出多张时在名字后面接一个
/// 从 1 起的序号（`001.jpg` → `001-1.png`、`001-2.png`），序号顺序就是阅读顺序。
///
/// 加了序号的名字可能撞上卷里本来就有的另一个成员（源里同时有 `001.jpg` 与 `001-1.png`），
/// 那一撞由 [`ensure_one_member_per_output`] 当场拦下，不静默覆盖。
fn output_name(relative: &Path, ordinal: usize, count: usize) -> PathBuf {
    if count <= 1 {
        return relative.with_extension("png");
    }
    let mut name = relative.file_stem().unwrap_or_default().to_os_string();
    name.push(format!("-{}.png", ordinal + 1));
    relative.with_file_name(name)
}

/// 一个源页产出的那几张输出页的成员名，按阅读顺序。规则见 [`output_name`]。
fn output_names(relative: &Path, count: usize) -> Vec<PathBuf> {
    (0..count)
        .map(|ordinal| output_name(relative, ordinal, count))
        .collect()
}

/// 这一卷每个源页**当它一张都不切时**的输出成员名：外层按源页序，内层按阅读顺序。
///
/// 这一份是碰像素之前唯一给得出来的名单：一个源页产出几张由内容决定（页几何批 04 号票），
/// 而这一步在解码之前。它只喂开工前那道撞名校验——拦下与内容无关的那些
/// （`001.jpg` 与 `001.png` 撞在同一个输出上、归档里的同名成员），
/// 买的是**别白做一整卷**。真正产出的那批名字等第一遍走完再查一遍
/// （见 [`ensure_no_two_outputs_collide`]）。
///
/// 幂等不再问它：名单改从上一趟写在输出里的记录读回来（见 [`compare_with_the_prior_output`]）。
///
/// 透传文件原名不动，不必单列一份。
fn one_to_one_targets(volume: &Volume) -> Vec<Vec<PathBuf>> {
    volume
        .pages
        .iter()
        .map(|page| output_names(&page.relative, 1))
        .collect()
}

/// 轮到这一卷时它的卷根还在不在。不在就是**这一卷没做成**（05 号票）。
///
/// # 为什么非得单问一句
///
/// 卷根整个消失走不到别的任何一条卷级失败上。读不出字节的成员在幂等那一道被记成
/// 「读不出来」照样喂进哈希（见 [`volume_fingerprint`]），在第一遍里变成**失败页**
/// （p0 的 12 号票），于是整卷成了一沓占位白页进隔离目录：报告说得出「这一卷全是坏页」，
/// 却不说它压根不在了，退出码也停在 `2`。那不是一卷做出来了。
///
/// # 判据是**卷根还在不在**，不是「成了几页」
///
/// 「一页都没成」与「卷根不在」是两件事，落在两个退出码上：
///
/// - 卷根在，而里面每一页都读不出来、解不出来 → 页级失败照旧，整卷进隔离目录（`2`）。
///   那一卷**做出来了**：页序、卷内统一尺寸、透传文件都在，坏的是内容
///   （`tests/isolation.rs` 的 `a_volume_whose_every_page_fails_still_comes_out_whole`）。
/// - 卷根不在 → 卷级失败（`3`），报告指得出是哪一卷、说得出为什么。
///
/// 拿「成功页数为 0」当判据会把前一种一起收走，而那一种正是 p0 的 12 号票定死的东西：
/// 一页读不出来不毁掉整卷。本票改的是它上面一层，不是推翻它。
///
/// # 两种容器形态都问
///
/// 问的是**读取层真会去走的那条路**，而两种形态走的是同一条：卷在预扫里已经放掉，
/// 轮到它时按路径重开（见 `survey`）——路径不在，[`source::open`] 第一步就走不下去。
///
/// 归档卷从前不问：它的字节从预扫时就打开、此后一直握着的那个句柄里出，卷根被删掉
/// 也照读不误，报「没做成」是撒谎。预扫不再攥着那个句柄，这条豁免跟着没了
/// （`volume-discovery/01`）。
///
/// 那一版里 Q50 那个缺陷因此只在目录卷这一侧；现在两侧同形，一句话说得完两种。
///
/// # 问在这里，也只问这一次
///
/// 问在**这一卷的第一件事上**：往下每一步都要读它的字节，卷根不在的话那几步全是白工——
/// 重开一次、幂等把整卷哈希一遍、第一遍逐页解一遍、第二遍再写一整卷白页出去。
///
/// 重开那一次**自己也会失败**，因此这一问买的不是「早一步发现」，是**那句话**：
/// 不问的话报出来的是 `source::open` 的「X 不存在」，说不出「它是在预扫之后不见的、
/// 不是它里面的页坏了」。两者落在同一个退出码上，读的人却只在后一句里看得出发生了什么。
///
/// 卷根是在这一问**之后**才消失的那一种仍旧落回页级失败那条路。一次 stat 拦不住这种竞态，
/// 而管线也无处去拿「整卷读到一半才发现根没了」这个事实——那时它手上只有一页一页读不出来，
/// 与「这一卷的页全坏了」长得一模一样。
///
/// 「探不到」与「不在」都算这一卷做不成：权限变了、盘拔了、路径中间少了一节，
/// 三样都到不了这一卷的字节，而这一层能说的正是这一句。
fn ensure_the_volume_root_is_still_there(root: &Path) -> Result<()> {
    let there = std::fs::exists(root).with_context(|| format!("查 {} 还在不在", root.display()))?;
    if !there {
        bail!(
            "{} 在预扫之后不见了：这一卷没做成——不是它里面的页坏了",
            root.display()
        );
    }
    Ok(())
}

/// 每个输出成员只对一个源成员。
///
/// 扩展名一律换成 png，`001.jpg` 与 `001.png` 于是撞在同一个输出上；一个源页产出多张时
/// 加的那个序号也可能撞上卷里本来就有的成员；归档里还可能有同名成员。
/// 撞了就报错——静默覆盖会让 `Report` 里两页指向同一个文件。
fn ensure_one_member_per_output(volume: &Volume, targets: &[Vec<PathBuf>]) -> Result<()> {
    let pages = volume
        .pages
        .iter()
        .zip(targets)
        .flat_map(|(page, names)| names.iter().map(move |name| (page, name.as_path())));
    let extras = volume
        .extras
        .iter()
        .map(|extra| (extra, extra.relative.as_path()));
    ensure_distinct_outputs(pages.chain(extras), |member| {
        volume.identity(member).display().to_string()
    })
}

/// 第一遍**真产出**的那批成员名互不冲突（页几何批 04 号票）。
///
/// 开工前那一遍（[`ensure_one_member_per_output`]）只查得了一对一那套名字：切开之后
/// 加的那个序号可能撞上卷里本来就有的成员——源里同时有 `001.jpg` 与 `001-1.png`，
/// 前者被切成两张，`001-1.png` 就有两个主人。那一撞要在**写出第一个字节之前**拦下，
/// 而这里正是两批名字第一次同时在手上的地方（第二遍还没开始，输出容器还没建）。
///
/// 报错指得出是哪两个源成员：输出页自己记着它来自哪一张（[`OutputPage::source`]），
/// 而透传成员按原名占着位。**留下的页一并查**（two-pass-rework/14）：它们同样要占输出里的一格，
/// 新切出的 `001-1.png` 撞上留下的同名一张，与撞上源里的同名成员是同一回事。
fn ensure_no_two_outputs_collide(volume: &Volume, pages: &[Slot]) -> Result<()> {
    let written = pages.iter().map(|page| (page.source(), page.target()));
    let extras = volume
        .extras
        .iter()
        .map(|extra| (volume.identity(extra), extra.relative.clone()))
        .collect::<Vec<_>>();
    let extras = extras
        .iter()
        .map(|(identity, relative)| (identity.as_path(), relative.as_path()));
    ensure_distinct_outputs(written.chain(extras), |source| source.display().to_string())
}

/// 一批 (源成员, 它要写到的输出成员) 里有没有两个源成员认领同一个输出成员。
///
/// 同一个源成员出现几次是合法的——它产出几张输出页就出现几次（页几何批 03 号票）；
/// 同一个输出成员被两个源成员认领则不行。
///
/// `identity` 只在真撞上时才被叫到：报错要指得出是哪两个源成员，而拼那两个名字
/// 不该让每一个卷都白付一遍。源成员做成类型参数而不是钉死 `&Member`，买的是这道校验
/// **单独测得了**——`Member` 的归档序号是 `source` 模块的私有字段，卷外造不出一个来，
/// 而一对多的撞名恰恰要在卷外喂进去才试得到（见本文件的用例）。
fn ensure_distinct_outputs<'a, M: Copy>(
    pairs: impl IntoIterator<Item = (M, &'a Path)>,
    identity: impl Fn(M) -> String,
) -> Result<()> {
    let mut taken: HashMap<&Path, M> = HashMap::new();
    for (member, relative) in pairs {
        if let Some(previous) = taken.insert(relative, member) {
            bail!(
                "{} 与 {} 都要写到 {}：请让同一卷内的成员名互不冲突",
                identity(previous),
                identity(member),
                relative.display()
            );
        }
    }
    Ok(())
}

/// 源库只读（ADR 0009）：输出与源卷互相嵌套时直接拒绝，不去猜用户的意思。
/// 两个卷不能写到同一个地方。
///
/// 输出镜像源的结构（ADR 0014 决定第 4 条），因此**同一棵树底下的卷自己就分得开**——
/// 一个作品目录下四个「第01话.cbz」各自躺在各自的作品目录里，镜像出来也各在各的一级。
/// 撞得上的只剩两种：
///
/// - **一次点名了多个地方**，两边各有一部叫「第 1 话」的卷。后到的会把先到的**整卷盖掉**
///   ——一句告警都没有，在阅读器里也与真卷毫无分别。
/// - **归档卷的扩展名归一**（ADR 0015），同一目录下的 `第10话.zip` 与 `第10话.cbz`
///   落到同一个去处。
///
/// 同一道拒绝管两种来由，而**那句话按撞上的那几组现拼**：出路不同——点名多处撞车分批处理
/// 就分得开，扩展名归一撞的这一对分不开——一句把两条出路都念出来，对其中一种必然是错的指引
/// （判据见 [`normalises_an_extension`]）。
///
/// **查的是发现出来的那些卷**，不是点名的那几个路径：点名的是「在哪里找」，
/// 不是「找到什么」。这一道因此排在预扫之后（见 `run`），撞车仍在写出第一个字节之前说。
///
/// 不替用户改名。「输出名就是卷名」这条约定要能反着用——看着输出得认得出是哪一卷——
/// 自动加后缀会让它失效，而失效的方式还是静默的。
fn ensure_no_two_volumes_share_an_output(
    volumes: &[survey::Surveyed],
    output_root: &Path,
) -> Result<()> {
    let mut by_target: HashMap<String, (PathBuf, Vec<&Path>)> = HashMap::new();
    for surveyed in volumes {
        let target = surveyed.output_path(output_root);
        by_target
            .entry(collision_key(&target))
            .or_insert_with(|| (target, Vec::new()))
            .1
            .push(surveyed.root.as_path());
    }
    let mut collisions: Vec<_> = by_target
        .into_values()
        .filter(|(_, by)| by.len() > 1)
        .collect();
    if collisions.is_empty() {
        return Ok(());
    }
    // 顺序取自第一个卷的路径：报错要可复现，而 `HashMap` 的遍历序不是。
    collisions.sort_by(|(_, a), (_, b)| a[0].cmp(b[0]));

    let total: usize = collisions.iter().map(|(_, by)| by.len()).sum();
    let mut said =
        format!("{total} 个卷要写到同一批去处，后到的会把先到的整卷盖掉。撞在一起的是：\n");
    // **抬头数的是卷、收口数的是「处」**：一处撞车至少两个卷，两个数因此对不上，
    // 而两者说的正是两件事——盖掉的是卷，要分开处理的是撞在一起的那几处。
    // 列几条、剩下的怎么说走的是[那一处出处](FirstFew)。
    said.push_str(&FirstFew::of(&collisions).stacked(
        |(target, group)| {
            let mut entry = format!("  {}\n", target.display());
            for root in group {
                entry.push_str(&format!("    ← {}\n", root.display()));
            }
            entry
        },
        "处",
    ));
    // 两条出路各按自己那一种撞车出场：混着念，对其中一种必然是错的指引。
    let by_volume_name = collisions
        .iter()
        .any(|(_, by)| !normalises_an_extension(by));
    let by_extension = collisions.iter().any(|(_, by)| normalises_an_extension(by));
    said.push_str("输出按源的结构镜像，末一级取自卷名，同名的卷因此撞在一起。");
    if by_volume_name {
        said.push_str("分批处理，每批给一个自己的输出根。");
    }
    if by_extension {
        said.push_str(&format!(
            "\n上面有一对只差扩展名：归档卷的输出扩展名一律归一成 .{}，源那一头叫什么\
             扩展名都不带过来，两份包因此指着同一个去处。这一对换输出根分不开\
             ——卷名本来就相同——只点名其中一份。",
            source::OUTPUT_ARCHIVE_EXTENSION
        ));
    }
    bail!(said)
}

/// 这一组撞在一起的卷里，有没有**扩展名归一**的份。
///
/// 判据是**文件名不同**。卷名撞车的两个源文件名必然相同（`甲部/第1话` 与 `乙部/第1话`
/// 都叫 `第1话`）；文件名不同还撞得上同一个去处，只可能是归档卷的扩展名在
/// [`source::output_name_of`] 那一步被归一掉了（`第10话.zip` 与 `第10话.cbz`）。
///
/// 比文件名用的是 [`collision_key`]：与比去处同一把尺子，不然 Windows 上
/// `第1话.CBZ` 与 `第1话.cbz` 会被这里当成两个名字、报成扩展名归一，而它撞的其实是大小写。
fn normalises_an_extension(group: &[&Path]) -> bool {
    let mut names = group
        .iter()
        .map(|input| collision_key(Path::new(input.file_name().unwrap_or(input.as_os_str()))));
    let Some(first) = names.next() else {
        return false;
    };
    names.any(|name| name != first)
}

/// 撞车比的是文件系统认不认成同一个去处。
///
/// Windows 上大小写不区分，`Abc.cbz` 与 `abc.cbz` 是同一个文件；别的平台上是两个。
/// 按平台折叠，查出来的撞车才与真会发生的撞车一致。
fn collision_key(target: &Path) -> String {
    let text = target.to_string_lossy().into_owned();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

fn ensure_output_is_elsewhere(input: &Path, output_root: &Path) -> Result<()> {
    let input_path = resolve(input)?;
    let output_path = resolve(output_root)?;
    if output_path.starts_with(&input_path) || input_path.starts_with(&output_path) {
        bail!(
            "输出目录 {} 与源卷 {} 相互嵌套：源库只读，请把输出写到别处",
            output_root.display(),
            input.display()
        );
    }
    Ok(())
}

/// 输出根写不写得进，**开工前探一次**（06 号票）。
///
/// `--out` 指到一个建不出来的路径，错在这一趟的**参数**上，换一个卷不会变好——
/// 按 `CONTEXT.md` 的《失败》那就是**拒绝执行**：整趟当场停，一页都不做，那句话只说一次。
/// 不问的话它由 [`Sink::create`] 在每一卷里各撞一次（停车场 Q51）：报出来的东西没骗人，
/// 但本该说一次的话说了 N 遍，而那时的退出码是「有卷没做成」（`3`），
/// 含义弱于本该给的那个 `1`。
///
/// 出来的错误**不戴 [`Refusal`]**：那件外套认的是从 [`process_volume`] 里出来的错误——
/// 一卷做不成与整趟别做了在那条路上长得一样。开工前这几道检查的错误由 `run` 直接返回，
/// 调用方拿到 `Err` 本身就是「这一趟没做成」（退出码 `1`），没有第二种可能要分辨。
///
/// # 只答「输出根写得进吗」
///
/// 拦下的只有**输出根**这一个、对每一卷都一样的事实。逐卷现建输出容器那条路一个字没改：
/// 某一卷的去处被占、单卷权限不同都到得了那里，那些是真的「这一卷做不成」，
/// 仍走卷级失败（`3`），其余卷照做。
///
/// # 为什么非得真写一次
///
/// 「这个目录写得进吗」没有一个可移植的问法：`std::fs::Permissions::readonly` 读的是
/// 只读**属性**，Windows 上的目录根本不认它；ACL、只读挂载、网络共享、配额，
/// 一样都不在它的视野里。文件系统只在真被写的时候才答这个问题，这里因此就真写一次。
///
/// # 探完盘上一个字节都不多
///
/// 现建的那几级目录记在手上，探完按自底向上收回去。
/// 这一道因此**不看模式**：[`Mode::DryRun`] 那句「一个文件都不落盘」说的是**输出**——
/// 页、记录、输出容器，也就是用户留得下的那些东西；探针留不下任何东西，
/// 探完的盘与没探过逐字节一样。
///
/// 反过来，试算要是答不出「你这个 `--out` 根本用不了」，那份报告就在撒谎：拒绝执行说的是
/// **参数**，而两种模式的参数是同一份，另外几种拒绝也都在试算里照样咬人
/// （`CONTEXT.md` 的《会话》：试算与执行是同一个 `run`，区别只在 mode）。
///
/// # 它拦不住什么
///
/// 探得过而真写时才坏的那些——盘满，或者探过之后输出根被删、权限被改——仍旧落回
/// **卷级失败**（`3`），与 [`ensure_the_volume_root_is_still_there`] 那一处的竞态同一个形状：
/// 一次探测拦不住时间。这一道要的不是「此后一定写得出去」，
/// 是**开工前就知道写不出去的那一种别再一卷一卷地撞**。
///
/// 同一条时间上还有两笔**认下的代价**，都窄，也都没有便宜的躲法。一是「哪几级要现建」是
/// 探之前记下的：另有人恰在那之后把同名目录建起来，收拾那一步会把它删掉——删得掉的至多是
/// 一个空壳（`remove_dir` 只删空目录），而两趟 tonefit 同时探同一个输出根时，
/// 被删的那一趟随后自己再建一遍。二是进程正好在试写与删探针之间被杀：
/// 那个点开头的探针文件会留在输出根里。躲开两者要一份日志或一把跨进程的锁，
/// 而那比它们值钱得多。
fn ensure_the_output_root_takes_a_write(output_root: &Path) -> Result<()> {
    // 要现建的是哪几级，**深的在前**——收拾时按这个次序走一遍正好是自底向上。
    // 问的是 `symlink_metadata` 而不是 `exists`：后者跟着符号链接走，一条**断了的**链接
    // 于是答「这里没东西」，而它明明占着这个名字，收拾那一步就会去删一个不是自己建的东西。
    // 已经占着的那一级就此打住，哪怕它是个文件——那时该失败的是下面那句 `create_dir_all`。
    let mut made = Vec::new();
    let mut level = Some(output_root);
    while let Some(path) = level {
        // 相对路径走到头是一个空路径（`Path::new("out").parent()` 是 `Some("")`）：
        // 它不是一级目录，既建不出来也收拾不着，不该记下来。
        if path.as_os_str().is_empty() || path.symlink_metadata().is_ok() {
            break;
        }
        made.push(path.to_path_buf());
        level = path.parent();
    }

    // 两句交代里都不再提一遍路径：它们只被下面那句 `bail!` 收走，而那一句已经把输出根
    // 指名道姓说过了。再提一遍就是同一个路径在一行里出现两次。
    let probed = std::fs::create_dir_all(output_root)
        .context("建出这个目录")
        .and_then(|()| {
            // 探针名带着进程号：两趟 tonefit 同时探同一个输出根时各删各的那一个，
            // 谁也不会把对方刚建出来的探针删掉、再据此判自己写不进去。
            let probe = output_root.join(format!(".tonefit-{}.probe", std::process::id()));
            let written = std::fs::write(&probe, b"").context("往这个目录里试写一个文件");
            // 写成了就删得掉，没写成它本来就不在——两种都不该盖过上面那个答案。
            let _ = std::fs::remove_file(&probe);
            written
        });

    // 收拾**不许**被 `?` 跳过：探不过的那一趟同样要把自己建出来的那几级收回去，
    // 而早返回会把它们留在盘上。答案因此攒在 `probed` 里，收拾完了再交出去。
    for path in &made {
        match std::fs::remove_dir(path) {
            Ok(()) => {}
            // `create_dir_all` 半路失败时上面那几级根本没建出来，接着往上收。
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            // 建出来了却删不掉——同一刻另有人往里放了东西。就此打住：
            // 它的上一级也已经不空了，再删只会失败。
            Err(_) => break,
        }
    }

    if let Err(error) = probed {
        bail!(
            "输出根 {} 写不进去：{error:#}。             这一趟的每一卷都要写到它下面，换一个卷不会变好——一页都没做，请换一个 --out",
            output_root.display()
        );
    }
    Ok(())
}

/// 规范化到可比较的绝对路径。输出目录还不存在，因此上溯到最近的已存在祖先再接回剩下的分量。
fn resolve(path: &Path) -> Result<PathBuf> {
    let absolute =
        std::path::absolute(path).with_context(|| format!("解析路径 {}", path.display()))?;
    let mut suffix = Vec::new();
    let mut current = absolute.as_path();
    loop {
        if let Ok(canonical) = current.canonicalize() {
            let mut resolved = canonical;
            resolved.extend(suffix.iter().rev());
            return Ok(resolved);
        }
        match (current.file_name(), current.parent()) {
            (Some(name), Some(parent)) => {
                suffix.push(name.to_os_string());
                current = parent;
            }
            // 一个已存在的祖先都没有，退回词法绝对路径。
            _ => return Ok(absolute),
        }
    }
}

#[cfg(test)]
mod tests {
    //! 一个源页产出**两张**输出页时，下游还跟不跟得上（页几何批 03 号票）。
    //!
    //! 这几条直接喂一份 N=2 的第一遍产物，问的是从成员命名到进度步数、从汇总到写出，
    //! 这条管线认不认这个形状。上界是 [`MAX_OUTPUTS_PER_SOURCE_PAGE`]。
    //!
    //! 与集成用例分工不同：`tests/` 那一批经 `run` 这个 seam，走的是跨页拆分真判出来的 N
    //! （页几何批 04 号票落地后命令行造得出 N=2）；这里绕开那套判定直接给形状，
    //! 因此拆分的规则怎么变，这几条问的东西都不变。

    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    use crate::cache::Retention;
    use crate::source::Container;

    /// 基准 profile 上门成立的那一页拿得到的候选，由小到大。
    const CANDIDATES: [(BitDepth, Dither); 3] = [
        (BitDepth::One, Dither::Off),
        (BitDepth::Two, Dither::Off),
        (BitDepth::Four, Dither::Off),
    ];

    /// 一份最小的请求。各用例只改自己那一处。
    ///
    /// `pub(crate)` 是给 `survey` 那几条用例的：它们要的也是这一份，只改点名的卷与输出根。
    pub(crate) fn request() -> Request {
        Request {
            inputs: vec![PathBuf::from("library/volume-a")],
            output_root: PathBuf::from("out"),
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            fit: FitMode::default(),
            crop: true,
            split: SplitRule::default(),
            filter: Filter::default(),
            white_align_limit: WhiteAlignLimit::default(),
            bit_depth: None,
            dither: None,
            envelope: false,
            cache_budget: CacheBudget::default(),
            mode: Mode::Process,
            io_mode: IoMode::default(),
            progress: None,
            metadata: true,
        }
    }

    /// 按卷级那条路去比上一趟的输出：整卷跳过答 `Some(输出页数)`，否则 `None`。
    ///
    /// 走的是卷级那条路，因此答不出「按页」——本文件里那几条钉的是
    /// 卷级那一问（`p0-hardening/03`、页几何批 04 号票），按页那一问在
    /// `tests/idempotency.rs` 上测。
    fn whole_skip(output: &Path, volume: &Volume, fingerprint: &Fingerprint) -> Option<usize> {
        let written = sink::Written::open(output, volume.container);
        match compare_with_the_prior_output(written, volume, fingerprint, &sink::Lodgers::default())
        {
            Reuse::Whole { page_count } => Some(page_count),
            Reuse::ByPage { .. } => panic!("卷级那条路上不该答按页"),
            Reuse::Nothing => None,
        }
    }

    /// 卷级那条路上的一份指纹：全卷那一个数由 `digit` 重复而成，用例要的只是「一个固定的数」。
    fn by_volume(digit: char) -> Fingerprint {
        let mut hasher = metadata::SourceHasher::new();
        hasher.member(Path::new("volume"), digit.to_string().as_bytes());
        Fingerprint::new(&request(), SourceHash::Volume(hasher.finish()))
    }

    /// 一张走灰度路径的输出页。`values` 是三个候选各自的判据值。
    fn gray(source: &str, target: &str, size: Size, values: [f32; 3], slot: usize) -> OutputPage {
        let scores = CANDIDATES
            .into_iter()
            .zip(values)
            .map(|((bit_depth, dither), value)| CandidateScore {
                candidate: Candidate::new(bit_depth, dither),
                score: Score::from_value(value),
            })
            .collect();
        OutputPage {
            source: PathBuf::from(source),
            target: PathBuf::from(target),
            origin: Some(Origin::new(Path::new(source), 0, 1)),
            outcome: Outcome::Processed {
                size,
                crop: Crop::keeping_all(size),
                backstopped: false,
                cut: None,
                spread_candidate: false,
                scaling: Scaling::plan(size, size),
                color: PageColor::Gray,
                branch: Branch::Gray {
                    scores,
                    gate: GeometryGate::Holds,
                    slot,
                    // 这一组用例问的是形状与序列，不是纸白：点名关掉那一趟的取值。
                    white: WhiteAlignment::Off,
                },
                salvage: None,
            },
        }
    }

    /// 一张走彩色分支的输出页：没有判据曲线，也没有几何门。
    fn color(source: &str, target: &str, size: Size) -> OutputPage {
        OutputPage {
            source: PathBuf::from(source),
            target: PathBuf::from(target),
            origin: Some(Origin::new(Path::new(source), 0, 1)),
            outcome: Outcome::Processed {
                size,
                crop: Crop::keeping_all(size),
                backstopped: false,
                cut: None,
                spread_candidate: false,
                scaling: Scaling::plan(size, size),
                color: PageColor::Color,
                branch: Branch::Color { encoded: None },
                salvage: None,
            },
        }
    }

    /// 一对一那条老路一个字符都不改，一对多才加序号（页几何批 03 号票的成员命名）。
    ///
    /// 前半句是这张票「什么都没变」那条验收在命名这一侧的形式：产出一张时名字
    /// 就是源页名换扩展名，升级的人手上的输出不会有成员被改名。
    #[test]
    fn one_output_keeps_the_source_name_and_several_get_ordered_suffixes() {
        assert_eq!(
            output_names(Path::new("001.jpg"), 1),
            [PathBuf::from("001.png")]
        );
        assert_eq!(
            output_names(Path::new("ch1/001.jpg"), 2),
            [
                PathBuf::from("ch1/001-1.png"),
                PathBuf::from("ch1/001-2.png")
            ]
        );
        // 序号从 1 起，序号顺序就是阅读顺序。
        assert_eq!(
            output_names(Path::new("001.png"), 3),
            [
                PathBuf::from("001-1.png"),
                PathBuf::from("001-2.png"),
                PathBuf::from("001-3.png")
            ]
        );
    }

    /// 写出那一段按**输出**成员数，读那两段按源那一侧（页几何批 03 号票的进度步数）。
    ///
    /// 分得开才要紧：读源与解源页都发生在切开之前，只有写出那一段跟着切完的张数走。
    /// 混成一个数的话，切开的卷进度条会在第二遍里走过头或者停下不动。
    ///
    /// **摊开那一段**（`p4-parking-lot/13`）在末尾单问一次：它与源那一侧数的是同一批成员，
    /// 却由格式定在不在，因此拿一个不摊开的卷与一个摊开的卷对着看。
    #[test]
    fn the_write_segment_counts_output_pages_and_the_read_segments_count_source_pages() {
        // 三张源页切成五张输出页，外加一个透传文件：幂等读 3+1、第一遍走 3、第二遍写 5+1。
        let split = MemberCounts {
            source_pages: 3,
            output_pages: 5,
            extras: 1,
            extracted_members: 0,
        };
        assert_eq!(volume_steps(split, &request()), 4 + 3 + 6);
        // 一对一时与从前逐字相同。
        let intact = MemberCounts {
            output_pages: 3,
            ..split
        };
        assert_eq!(volume_steps(intact, &request()), 4 + 3 + 4);
        // dry-run 没有第二段，切成几张都不改变步数。
        let dry = Request {
            mode: Mode::DryRun,
            ..request()
        };
        assert_eq!(volume_steps(split, &dry), 4 + 3);
        // `--no-metadata` 关掉幂等那一段，第二段照旧按输出算。
        let bare = Request {
            metadata: false,
            ..request()
        };
        assert_eq!(volume_steps(split, &bare), 3 + 6);
        // 固实归档多走一段：整卷四个成员先摊到临时目录，其余三段一格不动。
        let extracted = MemberCounts {
            extracted_members: 4,
            ..split
        };
        assert_eq!(volume_steps(extracted, &request()), 4 + 4 + 3 + 6);
        // dry-run 也照样摊开——摊开在开工前，与写不写输出无关。
        assert_eq!(volume_steps(extracted, &dry), 4 + 4 + 3);
    }

    /// 一个输出成员只对一个源成员：同一源成员出现几次是合法的，两个源成员撞在一起不行。
    ///
    /// 加了序号的名字可能撞上卷里本来就叫那个名字的成员——`001.jpg` 切出来的 `001-1.png`
    /// 与一张真叫 `001-1.jpg` 的页就是这个局面。撞了要当场指名道姓，不静默覆盖。
    #[test]
    fn two_source_members_may_not_claim_the_same_output_member() {
        // 同一个源成员产出两张，各占一个名字：合法。
        assert!(
            distinct(&[("001.jpg", "001-1.png"), ("001.jpg", "001-2.png")]).is_ok(),
            "一个源页产出多张不该被当成撞名"
        );

        let error = distinct(&[
            ("001.jpg", "001-1.png"),
            ("001.jpg", "001-2.png"),
            ("001-1.jpg", "001-1.png"),
        ])
        .expect_err("撞名该被拦下");
        let said = format!("{error:#}");
        for named in ["001.jpg", "001-1.jpg", "001-1.png"] {
            assert!(said.contains(named), "错误里没指出 {named}：{said}");
        }
    }

    /// 把 (源成员, 输出成员) 对喂给那道校验。源成员在这里就是一个名字。
    fn distinct(pairs: &[(&'static str, &'static str)]) -> Result<()> {
        ensure_distinct_outputs(
            pairs
                .iter()
                .map(|(source, target)| (*source, Path::new(*target))),
            |source: &str| source.to_owned(),
        )
    }

    /// 幂等要对上一个源页产出的**每一张**输出页，少一张就重做（页几何批 04 号票）。
    ///
    /// 名单不再是预告出来的：一个源页产出几张由内容决定，而这一道在解码之前。
    /// 它改由**上一趟写在输出里的记录**说出来——头一张写着「1/2」，第二张就非在不可
    /// （见 [`Origin`] 与 [`written_family`]）。
    ///
    /// 「输出里少一张就该察觉」这条能力是 `p0-hardening/03` 的地基，这一条钉的正是它：
    /// 只认「至少有一张」的实现在下半段会答「跳过」，而那一半再也补不回来。
    #[test]
    fn a_skip_needs_the_fingerprint_on_every_output_page_of_a_source_page() {
        let space = tempfile::tempdir().expect("建临时目录");
        // 一个真卷：一张源页，好让幂等那一道拿得到容器形态与透传清单。
        let root = space.path().join("volume-a");
        fs::create_dir_all(&root).expect("建源卷");
        let page = GrayImage::new(Size::new(4, 4), vec![128; 16]);
        fs::write(
            root.join("001.png"),
            encode::png(&page, BitDepth::One, None).expect("编一张源页"),
        )
        .expect("写源页");
        let volume = source::open_unwatched(&root).expect("打开源卷");

        // 这一张源页切成了两半：两张输出页各记着自己是那一族的第几张。
        let names = output_names(Path::new("001.png"), 2);
        let fingerprint = by_volume('0');
        let written = |ordinal: usize, count: usize| {
            let origin = Origin::new(Path::new("001.png"), ordinal, count);
            let record = Record::color(&fingerprint, &origin, None, None);
            encode::png(&page, BitDepth::One, Some(&record)).expect("编一张带记录的页")
        };

        // 两半都在、都带着这份指纹与自己那一格：跳得过，而且数得出是两张。
        let output = space.path().join("out-both");
        fs::create_dir_all(&output).expect("建输出容器");
        for (ordinal, name) in names.iter().enumerate() {
            fs::write(output.join(name), written(ordinal, 2)).expect("写一张输出页");
        }
        assert_eq!(
            whole_skip(&output, &volume, &fingerprint),
            Some(2),
            "两半都齐着还是重做了"
        );

        // 后一半被删掉：整卷重做。剩下那一张仍写着「1/2」，缺口因此看得见。
        fs::remove_file(output.join(&names[1])).expect("删掉后一半");
        assert_eq!(
            whole_skip(&output, &volume, &fingerprint),
            None,
            "输出里少了一张，这一卷仍然被跳过了"
        );

        // 旧记录（没有来路那一项）不命中：它说不出自己那一族有几张，
        // 证不了「输出里没少东西」，而幂等从不该给一个证不出来的命中。
        let old = space.path().join("out-old");
        fs::create_dir_all(&old).expect("建输出容器");
        let stale = encode::png(&page, BitDepth::One, None).expect("编一张不带记录的页");
        fs::write(old.join(&names[0]), &stale).expect("写一张输出页");
        fs::write(old.join(&names[1]), &stale).expect("写一张输出页");
        assert_eq!(whole_skip(&old, &volume, &fingerprint), None);
    }

    /// 没切开的那一族只有一张，而且它得**自己说是一张**（页几何批 04 号票）。
    ///
    /// 名字对上、指纹也对上，记录却写着「共两张」——那说明另一张要么被删了、
    /// 要么是别的参数跑出来的。这一族因此不齐，整卷重做。
    #[test]
    fn a_one_to_one_output_must_say_it_is_the_only_one() {
        let space = tempfile::tempdir().expect("建临时目录");
        let root = space.path().join("volume-a");
        fs::create_dir_all(&root).expect("建源卷");
        let page = GrayImage::new(Size::new(4, 4), vec![128; 16]);
        fs::write(
            root.join("001.png"),
            encode::png(&page, BitDepth::One, None).expect("编一张源页"),
        )
        .expect("写源页");
        let volume = source::open_unwatched(&root).expect("打开源卷");
        let fingerprint = by_volume('0');
        let written = |count: usize| {
            let origin = Origin::new(Path::new("001.png"), 0, count);
            let record = Record::color(&fingerprint, &origin, None, None);
            encode::png(&page, BitDepth::One, Some(&record)).expect("编一张带记录的页")
        };

        let honest = space.path().join("out-one");
        fs::create_dir_all(&honest).expect("建输出容器");
        fs::write(honest.join("001.png"), written(1)).expect("写一张输出页");
        assert_eq!(whole_skip(&honest, &volume, &fingerprint), Some(1));

        let lying = space.path().join("out-claims-two");
        fs::create_dir_all(&lying).expect("建输出容器");
        fs::write(lying.join("001.png"), written(2)).expect("写一张输出页");
        assert_eq!(whole_skip(&lying, &volume, &fingerprint), None);
    }

    /// 卷内统一尺寸的众数在**切开之后**取：同一源页的两半各算一张（页几何批 03 号票）。
    ///
    /// 按源页数，两个尺寸各一票、并列；按输出页数，切开的那个尺寸两票。
    /// 失败页照它留白占位，数错了整卷的占位页就换一个尺寸。
    #[test]
    fn the_uniform_size_counts_output_pages() {
        let narrow = Size::new(500, 800);
        let wide = Size::new(600, 800);
        let pages = [
            gray("001.jpg", "001-1.png", narrow, [9.0, 4.0, 1.0], 0),
            gray("001.jpg", "001-2.png", narrow, [9.0, 4.0, 1.0], 1),
            gray("002.jpg", "002.png", wide, [9.0, 4.0, 1.0], 2),
        ];

        assert_eq!(
            uniform_size(
                pages.iter().filter_map(OutputPage::size),
                Size::new(1264, 1680)
            ),
            narrow
        );
    }

    /// 汇总那一层的序号指进**输出页**那个序列，不是源页那个（页几何批 03 号票）。
    ///
    /// 头一张源页的两半走彩色分支、不进上包络，灰度页因此排在第 2、3 位上——
    /// 按源页数的话定档页会指到 0 或 1，而那是另一张页。
    #[test]
    fn the_volume_level_summary_indexes_output_pages_not_source_pages() {
        let size = Size::new(600, 800);
        let pages = vec![
            color("001.jpg", "001-1.png", size),
            color("001.jpg", "001-2.png", size),
            gray("002.jpg", "002-1.png", size, [9.0, 4.0, 1.0], 0),
            gray("002.jpg", "002-2.png", size, [8.0, 3.0, 0.5], 1),
        ];

        // 定档页只有上包络那条路才有，开着它问。
        let request = Request {
            envelope: true,
            ..request()
        };
        let (verdicts, verdict) = summarize_volume(&pages, &request);

        // 判定与输出页一一对应，一张一格。
        assert_eq!(verdicts.len(), 4);
        assert!(
            verdicts[0].is_none() && verdicts[1].is_none(),
            "彩色分支上没有判定"
        );
        assert!(verdicts[2].is_some() && verdicts[3].is_some());
        let Some(VolumeVerdict::Envelope(envelope)) = verdict else {
            panic!("这一卷该由上包络定档，实际是 {verdict:?}");
        };
        assert!(
            matches!(envelope.driver, 2 | 3),
            "定档页指到了第 {} 张，那不是一张灰度页",
            envelope.driver
        );
    }

    /// 同一源页切出来的两张各写各的成员，源那一格指着同一张源页（页几何批 03 号票）。
    #[test]
    fn both_halves_of_one_source_page_report_their_own_output_and_the_same_source() {
        let size = Size::new(600, 800);
        let verdict = Some(Verdict {
            candidate: Candidate::new(BitDepth::Two, Dither::Off),
            reason: Reason::VolumeEnvelope,
        });
        let out = Path::new("out/volume-a");

        let reports: Vec<PageReport> = vec![
            gray(
                "library/volume-a/001.jpg",
                "001-1.png",
                size,
                [9.0, 4.0, 1.0],
                0,
            ),
            gray(
                "library/volume-a/001.jpg",
                "001-2.png",
                size,
                [9.0, 4.0, 1.0],
                1,
            ),
        ]
        .into_iter()
        .map(|page| page.to_report(out, verdict, size))
        .collect();

        assert_eq!(reports[0].output, PathBuf::from("out/volume-a/001-1.png"));
        assert_eq!(reports[1].output, PathBuf::from("out/volume-a/001-2.png"));
        assert_eq!(
            reports[0].source, reports[1].source,
            "两半来自同一张源页，报告里那一格该指着同一个成员"
        );
    }

    /// 第二遍按输出页写出：一个源页的两半各落一个成员，各带各的字节，各报到一步（页几何批 03 号票）。
    ///
    /// 成员名跟着页走。由第二遍按源页序数出来的话，两半会写到同一个名字上——
    /// 一张覆盖另一张，而报告里两页仍各说各的。
    #[test]
    fn the_second_pass_writes_every_output_page_of_a_split_source_page() {
        let space = tempfile::tempdir().expect("建临时目录");
        let out = space.path().join("volume-a");
        let size = Size::new(4, 4);

        // 两半在缓存里各占一个序号，像素刻意不同：串位当场看得出来。
        let cache = Mutex::new(cache::PageCache::new(
            CacheBudget::default(),
            Retention::Keep,
        ));
        let slots: Vec<usize> = [0u8, u8::MAX]
            .into_iter()
            .map(|level| {
                let image = GrayImage::new(size, vec![level; 16]);
                lock(&cache)
                    .insert(cache::compress(&image))
                    .expect("存进缓存")
            })
            .collect();

        let pages = [
            gray(
                "volume-a/001.jpg",
                "001-1.png",
                size,
                [9.0, 4.0, 1.0],
                slots[0],
            ),
            gray(
                "volume-a/001.jpg",
                "001-2.png",
                size,
                [9.0, 4.0, 1.0],
                slots[1],
            ),
        ];
        let verdict = Some(Verdict {
            candidate: Candidate::new(BitDepth::Two, Dither::Off),
            reason: Reason::VolumeEnvelope,
        });
        let encode = Encode {
            uniform: size,
            cache: &cache,
            recorder: None,
        };
        let tally = Tally::default();
        let watching = ProgressSink::new(tally.clone());
        let standing = progress::Standing::default();
        let deliberation = progress::Deliberation::default();

        let mut sink =
            Sink::create(&out, Container::Directory, sink::Lodgers::default()).expect("建输出容器");
        let slots: Vec<Slot> = pages
            .iter()
            .map(|page| Slot::Redone { page, verdict })
            .collect();
        second_pass(
            &slots,
            &encode,
            &mut sink,
            None,
            progress::Events::new(Some(&watching), &standing, &deliberation),
        )
        .expect("写出这两张");
        sink.finish().expect("收尾");

        let mut members: Vec<String> = fs::read_dir(&out)
            .expect("列输出容器")
            .map(|entry| {
                entry
                    .expect("读目录项")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        members.sort();
        assert_eq!(members, ["001-1.png", "001-2.png"]);
        assert_ne!(
            fs::read(out.join("001-1.png")).expect("读回头一半"),
            fs::read(out.join("001-2.png")).expect("读回后一半"),
            "两半写出了同样的字节：缓存序号串位了"
        );
        // 一张输出页一步，不是一张源页一步。
        assert_eq!(tally.steps(), 2);
    }

    /// **哨兵真的炸**：持着卷缓存那把锁走到报到，当场恐慌，消息指得出是哪一处报到。
    ///
    /// 这条性质说的是「**没有**发生某件事」——读代码相信它是不够的。`p1-session/02`
    /// 逐个调用点核过一遍，但那是一次性的：下一个往管线里加报到的人漏了不会红、会死锁
    /// （停车场 Q40）。这里故意踩上去一次，问的就是「踩上去真的会红吗」。
    ///
    /// **两样都断言**：哪一条事件，以及**哪一行**。只比事件名的话，
    /// `#[track_caller]` 那一串掉了任意一处都不会红——消息会改口指进 `progress.rs`，
    /// 而「指得出是哪一处报到」当场落空，又回到靠人核。
    ///
    /// 踩法与真实的报到点无关：那五处一处都不该触发它，而**整个测试套件跑一遍**
    /// 就是那一侧的证据（哨兵不问有没有观察者，每一条走管线的用例都替它们验了一次）。
    ///
    /// 只在调试构建上跑：哨兵在发布构建上整个不在（见 [`CacheGuard`]）。
    #[test]
    #[cfg(debug_assertions)]
    fn reporting_a_step_while_holding_the_cache_lock_blows_up() {
        let fell_over = std::panic::catch_unwind(|| {
            let cache = Mutex::new(cache::PageCache::new(
                CacheBudget::default(),
                Retention::Keep,
            ));
            let watching = ProgressSink::new(Tally::default());
            let standing = progress::Standing::default();
            let deliberation = progress::Deliberation::default();
            let events = progress::Events::new(Some(&watching), &standing, &deliberation);

            let _held = lock(&cache);
            events.step();
        })
        .expect_err("持着缓存那把锁报到，哨兵该炸");

        let message = panic_message(fell_over);
        assert!(
            message.contains("在持着卷缓存那把锁的地方报到了：Stepped"),
            "消息没指出是哪一条事件：{message}"
        );
        assert!(
            message.contains(file!()),
            "消息指的不是报到那一行，`#[track_caller]` 那一串断了：{message}"
        );
    }

    /// **决策点也在哨兵里**，哪怕这一趟根本没有观察者可问。
    ///
    /// 它有自己一道判空（没人会读那份报告就连拼都不拼），绕得过 `Events::ask` 里那一问——
    /// 走这一支的是库外只调 `run`、不装观察者的那一趟。而这一处**按设计要等人**
    /// （ADR 0012 决定第 3 条），持着锁停在这儿是最坏的一种。
    #[test]
    #[cfg(debug_assertions)]
    fn the_decision_point_trips_the_sentinel_even_with_no_observer_to_ask() {
        let fell_over = std::panic::catch_unwind(|| {
            let cache = Mutex::new(cache::PageCache::new(
                CacheBudget::default(),
                Retention::Keep,
            ));
            let standing = progress::Standing::default();
            let deliberation = progress::Deliberation::default();
            let events = progress::Events::new(None, &standing, &deliberation);

            let _held = lock(&cache);
            events.ask_before_the_second_pass(|| unreachable!("哨兵该在拼报告之前就炸"))
        })
        .expect_err("没有观察者也该炸：漏掉的正是这一支");

        let message = panic_message(fell_over);
        assert!(
            message.contains("在持着卷缓存那把锁的地方报到了：PassStarted"),
            "消息没指出是哪一条事件：{message}"
        );
        assert!(
            message.contains(file!()),
            "消息指的不是报到那一行，`#[track_caller]` 那一串断了：{message}"
        );
    }

    /// 从一次恐慌里把那句话捞回来。哨兵那两条用例共用。
    #[cfg(debug_assertions)]
    fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
        *payload
            .downcast::<String>()
            .expect("哨兵恐慌时带的是一句话")
    }

    /// 数第二遍报到了几步。观察者那一端只关心「走完一步」这一种事件，
    /// 别的事件长什么样在 `progress` 与 `tests/events.rs` 里测。
    #[derive(Clone, Default)]
    struct Tally(Arc<AtomicUsize>);

    impl Tally {
        fn steps(&self) -> usize {
            self.0.load(Ordering::Relaxed)
        }
    }

    impl Progress for Tally {
        fn observe(&self, event: Event<'_>) -> Instruction {
            if matches!(event, Event::Stepped { .. }) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
            Instruction::Continue
        }
    }
}
