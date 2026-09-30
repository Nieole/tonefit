//! tonefit：把漫画页适配到电子墨水阅读设备。
//!
//! 对外是四个 seam，其余全部是内部实现。
//!
//! 根模块放这四个 seam 与**装配**：`run` 串起整趟，`process_volume` 串起一卷走过的那几个环节，
//! 连同装配自己要用的那几样——步数预告、开工前与卷内那几道校验、拒绝开始的标记。
//! 环节本身住在 `pipeline`（`CONTEXT.md` 的《管线》）。
//!
//! [`run`] 是主 seam：所有模式走同一个入口，CLI 是它之上的薄层，只负责把命令行参数拼成
//! [`Request`]、把 [`Report`] 渲染成文字。
//!
//! [`score`] 是第二个 seam：画质分的纯函数形态，数值与性质测试、标定工具直接调它。
//! 它周边的类型——[`Reference`]、[`Score`]、[`GrayImage`]、[`Candidate`]、[`quantize`]——
//! 一并公开，画质分的调用方要拿它们拼出参照与候选。
//!
//! [`write_calibration_chart`] 是第三个：灰阶测试图。它不并进主入口——不读源、不走管线、
//! 不判定，只按一个 [`Profile`] 画出一张图并**无损写到点名的那个文件上**。
//! 量具与被处理的页走的不是同一条路。
//!
//! [`write_proof`] 是第四个：样张。它同样不并进主入口，**理由却与第三个恰好相反**——
//! 它走的正是同一条路：一张图走满管线，每一个候选各编一张，判定那一档的那一张与 [`run`]
//! 不写记录时写出的逐字节相同。不并进去，是因为它**不是一次处理**：认的是一张图而不是卷，
//! 不判定别人、不写《记录》、不进幂等；而 [`Mode`] 的两个取值一个写出去、一个一个输出都不落盘，
//! 样张两样都不是——加第三个取值就是改写《模式》与《预览》两条词条的含义。
//!
//! 四个 seam 之外另有两样对外的东西，而它们不是缝，是**两条规矩**。
//! 两条都让库担了一点界面层的事，而**理由各是各的**——不要并成一条说，
//! 各自的理由写在各自那个模块上：
//!
//! - [`glyph`]——**字形约定**那两条（[`HARD_SPACE`] 与 [`width_is_stable`]，
//!   `CONTEXT.md`《字形约定》）。同一句话从三张嘴里出来、最后一张在库内
//!   （ADR 0016 认下的那处例外），库因此会把自己造的字**直接送进一条对齐的列**里
//!   ——规矩得跟着字一起递到调用方手上；
//! - [`listing`]——**只列前几条**那个形状（[`FirstFew`]，`CONTEXT.md`《只列前几条》）。
//!   一张长清单只列前几条、剩下的报个数，而说这一句的几张嘴里**有两张在库内**：
//!   清点那条拒绝与撞名那条拒绝，两段字都是库自己写下、直接落到 stderr 上的。
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
mod pipeline;
mod profile;
mod progress;
mod proof;
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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

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
pub use proof::{Proof, ProofPage, Sheet, Sheets};
pub use quantize::{BitDepth, Candidate, Dither, quantize};
pub use report::{
    NonVolumeFile, NonVolumeReason, PageBranch, PageOutcome, PageReport, Processed, Report,
    RunOutcome, UnreachablePlace, VolumeFailure, VolumeReport, VolumeTiming, VolumeVerdict,
};
pub use request::{Mode, Request};
pub use resample::{Filter, Scaling};
pub use spread::{Cut, Gutter, ReadingOrder, Side, SplitRule, SplitThreshold};
pub use survey::SurveyedVolume;
// 认得的归档扩展名那一串：命令行的 `--help` 也要说它，而格式集只有一个出处
// （`source::ARCHIVE_FORMATS`）。见二进制侧的 `inputs_help`。会话开跑之前那一副按扩展名
// 认文件夹还是压缩包，读的也是这一份（`is_archive`）。
pub use source::{is_archive, listed_archive_extensions};
pub use white::{WhiteAlignLimit, WhiteAlignment, align_white};

use metadata::Recorder;
use pipeline::{
    Candidates, Compute, ComputeCounters, Encode, FirstPass, OutputPage, Retained, Reuse, Settles,
    Slot, compare_with_the_prior_output, cores, driver, ensure_the_overrides_leave_a_candidate,
    first_pass, in_reading_order, lock, max_outputs_per_source_page, output_names, second_pass,
    summarize_volume, uniform_size, volume_fingerprint,
};
use sink::Sink;
use source::Volume;

/// 画一张灰阶测试图并写到 `out`，父目录不在就建出来。
///
/// 尺寸恒等于面板分辨率：图要在真机上 1:1 显示才答得准。
/// 它一次上机答两件事，先后印在图内——那两件事与为什么合在一张图上，见 `calibrate` 的模块文档。
///
/// **图本身不经过灰阶档位判定**：它是量具，不是被处理的页——画质分、整卷统一灰阶、抖动一概不碰它，
/// 像素以 8 位工作精度原样交给编码器，写出的是无损 PNG。自描述元数据也不写：
/// 记录说的是一页的判定与幂等依据（见 `metadata`），灰阶测试图两样都没有。
///
/// 落盘在库内完成，命令行与会话共用这一个调用（加固批 12 号票）：出图这件事从头到尾只有一份，
/// 界面层两边都不必自己建目录、自己写文件。写不出去时回的是 `Err`——盘满、
/// 父目录建不了都在里面，调用方接住它照自己的方式说，不必崩掉一整个会话。
///
/// 印在终端上的那几行不在这里：那是**界面文案**，随调用方走（见二进制侧的 `render`）。
pub fn write_calibration_chart(profile: &Profile, out: &Path) -> Result<()> {
    calibrate::write_chart(profile, out)
}

/// 出一张图的**样张**：走满管线，把这块面板上这一页派得出的**每一个**候选各编一张，
/// 连同《参照》一张写进 `out`，去处不在就建出来（`CONTEXT.md` 的《样张》）。
/// 彩色面板上的彩页例外，见下面《两种页拿不到整叠》。
///
/// 它是第四个 seam，为什么不并进 [`run`]、为什么又必须走同一批函数，见本模块文档。
/// 钉住后一句的是神谕那几条用例：**同一份 `request` 交给 [`run`]**（不写记录），
/// 写出去的那一张与这里判定那一档的那一张逐字节相同（`tests/proof.rs`）。
///
/// # `request` 读哪几格
///
/// 读的是**处理选项**那几格：型号、缩放方式、裁白边、拆分、缩放算法、提白上限，
/// 外加两道覆盖项（下一段）。
/// 卷级那几格——点名的卷、输出根、观察者、内存上限、读盘方式、写不写记录、做到哪一步——
/// **一格都不读**，一张图上它们无从谈起：样张恒不写《记录》，纸白恒读一遍
/// （上限取 0 时也读，见库内的 `WhiteWhenOff`）。整卷统一灰阶那一格也不读：那条路上的档要看完整卷
/// 才定得下，一张图给的是它自己那一档。
///
/// **两道覆盖项（`bit_depth`、`dither`）不裁样张的候选集**（样张 spec《Implementation Decisions》
/// 第三条）：它们裁掉的是「这一趟不要」，不是「这一页不可能」，而样张存在的理由正是并排看。
/// 出的是两道界（屏幕灰阶数、尺寸贴合检查）裁剩的那一整套；覆盖项只管判定从哪几个里挑——
/// 与转换那一趟逐格相同，顶死时理由是覆盖（哪一种算顶死，见 `CONTEXT.md` 的《覆盖顶死》）。
/// 「顶死没有」照转换那一趟默认那条路问（库内的 `Settles::if_processing`），不拿某一块剩下几个候选
/// 去问——门不成立的那一块只点灰阶档位也只剩一个，那不算顶死（停车场 Q1014）。
/// 覆盖项与面板对不上时（越界的灰阶档位、互锁 ③）回的是转换那一趟的同一句拒绝——
/// 越界的灰阶档位那一句彩色分支上的页也躲不过：转换那一趟碰卷之前就说它，样张也排在读图之前
/// （见下面《说不出话的那几种》）。
///
/// # 两种页拿不到整叠（`proof-sheet/05`）
///
/// 走哪条分支由**面板与页**共同决定，样张照转换那一趟那一处的判断走（库内的 `open_source_page`），
/// 不自己另判：
///
/// - **彩色面板上的彩页走彩色分支**，不量化（ADR 0005 决定第 4 条）：没有候选、没有画质分、
///   没有参照，那一叠只有**一张**——`run` 会写出的那一张（[`Sheets::Color`]），
///   与 [`run`] 不写记录时写出的逐字节相同。黑白面板上同一张彩页转灰，照灰度路径出整叠。
/// - **尺寸未贴合屏幕的页**候选里没有抖动那一维（ADR 0007 决定第 2 条）：出的是这一页的门
///   派得出的那一套，与 [`run`] 在同一页上用的那一套相同；门不成立交出来的数据说得出
///   （[`PageReport::gate`]）。
///
/// 交回这张图每一张输出页的那一叠（[`Proof`]）：拆开的跨页两半各一叠，关掉拆分、或是找不到中缝的
/// 连续跨页，整页一叠；每一叠的文件名，页那一截就是 `run` 给那一张的输出页名（库内的 `output_name`）。
/// 每一张落在哪儿、多大，连同这一页的分支、判定、几何事实与纸色提白。
///
/// # 说不出话的那几种（`proof-sheet/06`）
///
/// 答不出来时回 `Err`，一句话说清是哪一种，调用方接住它照自己的方式说：
///
/// - **点成别的东西**：一个目录、一个归档（转换那一趟的卷），或一个透传文件（扩展名不是页的成员）
///   ——一句「样张只认一张图」，一个字节都不读（停车场 Q1055）。路径根本不在的不算这一种，
///   由读盘那一步说它读不到。
/// - **解不开的图**：转换那一趟的**坏页**里解码那三种（`CONTEXT.md` 的《失败》：完整尺寸解不出来、
///   缓冲分配不下、一个像素都救不回），一句话说它解不开。残缺页不在里面，照出；
///   坏页的第四种（字节读不出来）在样张上就是读盘那一句。
/// - **写不出去**：去处建不出来、某一张写不进去（盘满、名字被占），一句话说样张写不出去、卡在哪个路径上。
///
/// **先全部编好再落盘**：覆盖项的拒绝排在最前（停车场 Q1042），前两种、撞上门、编不出来
/// 都发生在第一个字节写出去之前——去处里一个文件都没有，去处本来不在的话连目录都不建。
/// 写到一半才写不进去的，已经落下的那几张留在去处里（停车场 Q1056）。
///
/// 型号认不出来那一种不在这里：`request` 里的 [`Profile`] 由调用方解好，那一句只有一处出处
/// （[`Profile::resolve`]），命令行上转换、灰阶测试图、样张三条路说的是同一句。
///
/// 印在终端上的那几行不在这里：那是**界面文案**，随调用方走（见二进制侧的 `render`）。
pub fn write_proof(source: &Path, request: &Request, out: &Path) -> Result<Proof> {
    proof::write(source, request, out)
}

/// 在点名的若干路径底下**发现**卷，逐卷处理，产出设备优化副本。源库只读。
///
/// 点名的是**在哪里找**，不是**找到什么**：一个路径展开成它底下的那一批卷
/// （ADR 0014，见 `crate::discover`），输出按源的结构镜像到输出目录下。
///
/// # 两种失败分得开（05 号票）
///
/// **拒绝开始**回的是 `Err`：错在这一趟的**参数**上，换一个卷不会变好，整趟因此当场停
/// （见库内的 `Refusal` 与 `crate::survey`）。**哪几种算拒绝开始**，单子在
/// `CONTEXT.md` 的《失败》——那里连同「哪几种发生在开工之前」一起写着，这里不抄第二份。
///
/// 开工之前那几种一页都不做；**尺寸贴合检查那一种不是**——尺寸贴合检查是页的事实，
/// 要真撞上那一页才拦得住（见 `Candidates::for_gate`），那时先做完的卷已经在盘上，
/// 而调用方拿到的是错误、没有报告。那是这条路唯一说不出「一页都没做」的地方。
///
/// **卷转换失败**回的是 `Ok`：清点时打得开、轮到它却做不成的卷（**卷根整个不见了**、
/// 文件被删、盘拔了、权限变了、透传文件搬不动）记进 [`Report::failed_volumes`]，
/// 其余卷照做、报告照出。
/// 「一卷点不开就毁掉整趟」正是这条分岔要改掉的毛病——那时前面几十卷的输出还在盘上，
/// 而那份说得清它们是什么的报告全丢了。
///
/// **两样都不是**：发现出来的归档点不开、一页都没有的东西——它们连卷都不是，
/// 进的是 [`Report::non_volume_files`] 那张并列的第三张表，逐条带着路径与一句为什么，
/// 退出码一格不动（ADR 0014 决定第 3、5 条）。
///
/// # 无法访问的地方（`p4-parking-lot/11`）
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
    // 排在清点之前：它是开工前这几道里唯一往盘上写东西的，而清点要走一遍点名的那几棵树、
    // 把发现出来的每一个卷都枚举一遍，输出目录根本没地方落时那一趟是白付的（06 号票，见
    // [`ensure_the_output_root_takes_a_write`]）。撞名那一道排在清点**之后**——
    // 撞在一起的是发现出来的那些卷，发现之前问不出来（见
    // [`ensure_no_two_volumes_share_an_output`]）。
    ensure_the_output_root_takes_a_write(&request.output_root)?;
    // 硬盘类型**按路径**探测，一次运行共用一份缓存（ADR 0009 决定第 2 条，见 `medium`）：
    // 同一趟里源卷可能在仓库盘上、输出在系统盘上，逐卷各判各的，互不影响。
    let mut probes = medium::Probes::new();
    // 这一趟的事件流。闩活在这里——一次运行一份，`Request` 复用不到它
    // （见 [`progress::Events`] 的 `standing`）。在确认点上等人等掉的那一截同一条寿命。
    let standing = progress::Standing::default();
    let deliberation = progress::Deliberation::default();
    let events = progress::Events::new(request.progress.as_ref(), &standing, &deliberation);
    // **清点**：开工之前发现这一趟有哪些卷，把它们全枚举一遍，算出这一趟的全局总步数
    // （ADR 0011 决定第 3 条、ADR 0014）。它排在开工那条事件**之前**，因为那条事件要带着
    // 那个数；点名的坏路径因此在任何卷级事件之前就把整趟拒掉——输出目录下一个文件都没有
    // （见 `survey`）。
    let survey = survey::Survey::of(request)?;
    // 撞名要在写出第一个字节之前说，而**撞在一起的是发现出来的那些卷**——点名的是
    // 「在哪里找」，不是「找到什么」（ADR 0009 决定第 1 条）。这一道因此排在清点之后、
    // 开工那条事件之前。
    ensure_no_two_volumes_share_an_output(survey.volumes(), &request.output_root)?;
    // 开工前那几道检查与清点都排在它之前：那几种失败一条事件都不发，调用方拿到的是错误本身。
    // 报的是**发现出来的卷**，不是点名了几个路径：进度条上那个分母得是真要做的那些。
    // 清点的三份产出一起带出去（`session-redesign/03`）——它们此刻已经齐了，这里不另算。
    events.run_started(
        survey.steps(),
        &survey.roster(),
        survey.non_volume_files(),
        survey.unreachable_places(),
    );
    let mut volumes = Vec::with_capacity(survey.volumes().len());
    let mut failed_volumes = Vec::new();
    let mut outcome = RunOutcome::Completed;
    // 清点的**三份产出**一起交出来（见 `survey::Survey` 的那个同名方法）：卷这一份在下面
    // 被逐个吃掉，另两份原样挂到报告上。它们整份在开工之前就齐了——发现走完
    // 就不再变，因此按停止停在半路的那一趟，这两张表照样是全的。
    let (surveyed_volumes, non_volume_files, unreachable_places) =
        survey.into_volumes_and_the_rest();
    for surveyed in surveyed_volumes {
        // **卷边界上的检查点**（ADR 0013 决定第 1 条）：做完再停让当前卷跑完就停，
        // 而「当前卷跑完」正是这里——盘上因此只有完整的卷，下一趟幂等接着走。
        // 立即停止在这一道上与做完再停同样停下：力度更强的指令不该比更弱的那个停得更晚。
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
                // **立即停止**（ADR 0013 决定第 2 条）：这一卷停在页边界上、那格 `partial` 已经丢掉，
                // 它等于没做，报告里因此没有它这一条。下一卷更不必开工——卷边界那个检查点
                // 也会拦下它，这里明写是为了让「立即停止掉的卷不进报告」与「后面的卷不做」
                // 在同一处看得见。
                outcome = RunOutcome::of(events.standing());
                break;
            }
            // **拒绝开始**：错在这一趟的参数上，换一个卷不会变好（见 [`Refusal`]）。
            // 整趟当场停，返回的是那个错误本身——退出码 `1`，不是卷转换失败那个 `3`。
            // 结束那一条照发：开工报过了，结束就得报得到（见 `Event::RunFinished`）。
            Err(error) if error.downcast_ref::<Refusal>().is_some() => {
                events.run_finished(RunOutcome::Refused);
                return Err(error);
            }
            // **卷转换失败**（05 号票）：清点时打得开、轮到它却做不成的卷记一笔，
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
        // 在确认点上等人的那几分钟不算这一趟的账（停车场 Q41）：库那时一步都没走。
        // 各卷的 `VolumeTiming::elapsed` 各自减掉自己那一截，这里减的是全部卷的和。
        elapsed: started.elapsed().saturating_sub(events.deliberated()),
    })
}

/// **拒绝开始**：错在这一趟的参数上，不在这一卷上（`CONTEXT.md` 的《失败》）。
///
/// 卷转换失败与拒绝开始在 [`process_volume`] 的返回值上长得一样——都是 `Err`——
/// 而两者的处置正相反：前者记一笔、其余卷照做（退出码 `3`），后者整趟当场停
/// （退出码 `1`）。分辨它们的只有这个标记，`run` 靠 `downcast_ref` 认它。
///
/// **眼下只有一处**戴它：覆盖项把候选集裁空（见 [`candidates`](pipeline::candidates)
/// 与 [`why_nothing_is_left`](pipeline::why_nothing_is_left)）。
/// 其中互锁 ③ 那一支的处置明写着「维持拒绝」（页几何批 05 号票），而它撞得上的时机
/// 在分析环节里、一页一页地判（见 [`Candidates::for_gate`])——真落到卷转换失败那条路上，
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

/// 掐一段的表：跑一遍 `work`，把这一段的墙钟耗时写进 `segment`。`work` 回 `Err` 时也照写。
///
/// 写成一个函数而不是在调用处各写三行，为的是让「哪几段掐了表」一眼数得清：
/// 段与段不许重叠，而重叠一旦发生，[`VolumeTiming`] 里四段之和就会大于总耗时。
/// 四段的表都由 [`process_volume`] 交出去——后三段在它自己里面掐，摊开那一段交给
/// `source::open` 去掐（那一段夹在重开这一卷的中间，见那里的《摊开那一段的表》）。
/// 环节开工与掐表成对的那三段走 [`timed_pass`]。
fn timed<T>(segment: &mut Duration, work: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let value = work();
    *segment = started.elapsed();
    value
}

/// 走一个[环节](Pass)：先报它开工，紧接着掐它那一段的表（见 [`timed`]）。
///
/// **计时与进度同一条分界线**（`CONTEXT.md` 的《卷级计时》）靠的就是这两句挨着：
/// 那条事件报出去之后表才开始走，其间报的每一步都记在这个环节名下，直到下一个环节开工。
/// 摊开、查重、分析三个环节走它；写出环节不走——它开工那条事件是确认点
/// （[`progress::Events::ask_before_the_second_pass`]），等人那一截夹在报开工与掐表之间，
/// 不算进任何一段。
fn timed_pass<T>(
    events: progress::Events<'_>,
    pass: Pass,
    segment: &mut Duration,
    work: impl FnOnce() -> T,
) -> T {
    events.pass_started(pass);
    timed(segment, work)
}

/// 这一卷要走多少步（spec 的 story 30）。
///
/// **清点**算它，一卷一次（见 `survey`）：开卷那条事件报的是它，这一趟的全局总步数是
/// 它们的和。两个数因此不会分家——不是各算一遍，是加出来的。
///
/// 四段，一个[环节](Pass)一段，与 [`VolumeTiming`] 的四段是同一条分界线：
/// **摊开**落全部成员，幂等这一道读全部**源**成员，分析环节走每一张**源页**，
/// 写出环节写全部**输出**成员。
/// 源那一侧与输出那一侧不是同一个数——一个源页产出一到多张输出页（页几何批 03 号票），
/// 而几张由内容决定（有没有中缝，页几何批 04 号票）。四段里只有末一段按输出那一侧算：
/// 读源与解源页都发生在切开之前。
///
/// 各段自己可能不在——**摊开那一段只有固实归档有**（`.7z` / `.rar`，ADR 0015 决定第 3 条；
/// 画质分见 `source::Volume::extracts_before_work`），`--no-metadata` 关掉幂等那一段
/// （那时既没有记录可写也没有依据可比），dry-run 没有末一段（一个文件都不落盘）。
/// 因此按**这一趟真要做的事**算，而不是按一个固定的倍数：
/// 不然进度条会停在某个百分比上再也不动。
///
/// 摊开那一段是 `p4-parking-lot/13` 添的：那一段从前一步都不报，几百兆的卷在那里
/// 进度条一动不动。添进来的同时预告也跟着长，**「预告是上界」因此一格没动**——
/// 只报步不改预告的话，固实归档上进度条会冲过头。
///
/// 幂等命中的卷会提前收摊，那时走过的只有查重那一段（要摊开的卷上外加它前面的摊开）——
/// 预告的步数是**上界**，不是承诺，
/// 剩下的由 [`Event::VolumeFinished`] 一次性了结。**按页跳过的卷同理**（two-pass-rework/14）：
/// 留下的页分析环节不走，那几步少报；写出环节它们照样一步一张（搬也是写）。哪几页会留下
/// 要幂等那一道比过才知道，而清点在它之前，预告因此减不掉它们。
///
/// 写出那一段的数**也是上界**，理由与上面那条不同：一个源页产出几张要解了像素才知道，
/// 而这一步在解码之前。取的是[每个源页最多几张](pipeline::MAX_OUTPUTS_PER_SOURCE_PAGE)——
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
/// 数**两遍**（见 [`MemberCounts::of`]）：清点数一遍算出步数，处理那一卷时按重开的那一份
/// 再数一遍。报告里的数出自后一遍——它才是真做了的那一卷。
///
/// 几个数绑成一个类型而不是几个相邻的 `usize` 参数：它们总是一同算出、一同传下去，
/// 而几个同型的裸数换了位置编译器一句话都不会说，[`volume_steps`] 却会当场少报或多报一整段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MemberCounts {
    /// 源页数。幂等这一道读它们，分析环节走它们。
    source_pages: usize,
    /// 输出页数的**上界**。写出环节写它们——一个源页产出一到多张，切开发生在分析环节之内，
    /// 而几张由内容决定（页几何批 04 号票），因此这一步只给得出上界。
    output_pages: usize,
    /// 透传文件数。它不经切开，两侧数的是同一批。
    extras: usize,
    /// **开工前摊开**要落的成员数：固实归档是整卷（源页加透传），不摊开的卷是 0
    /// （`p4-parking-lot/13`）。
    ///
    /// 它与源那一侧数的是同一批成员，却单占一格：**那一段自己会不会走由容器与格式定**
    /// （见 `source::Volume::extracts_before_work`），而另外那几段在不在由这一趟的模式定
    /// （分析环节恒走，幂等那一道看 `--no-metadata`，写出环节看 dry-run）。
    /// 拿 `source_pages + extras` 在 [`volume_steps`] 里现算的话，
    /// 「这一卷摊不摊开」就得再传一个真假进去，而那正是这个类型存在的理由。
    extracted_members: usize,
}

impl MemberCounts {
    /// 数一个打开了的卷。
    ///
    /// 两个调用点各数各的那一遍：清点按它算这一卷的步数（见 `survey`），
    /// [`process_volume`] 按**重开的那一份**再数一次。公式因此只有一处——
    /// 两边各写一份的话，「预告了多少步」与「报告说做了多少页」会各自漂。
    fn of(volume: &Volume, request: &Request) -> Self {
        let source_pages = volume.pages.len();
        let extras = volume.extras.len();
        Self {
            source_pages,
            // 上界，不是承诺：一卷里真被切开的页越少，写出环节走过的步越少
            // （见 [`volume_steps`]）。
            output_pages: source_pages * max_outputs_per_source_page(request),
            extras,
            // **只问容器与格式，不看这一卷此刻摊开了没有**：清点数的那一遍卷还没摊开
            // （`source::enumerate` 交出来的读取端取不出字节），而两遍要数出同一个数。
            extracted_members: if volume.extracts_before_work() {
                source_pages + extras
            } else {
                0
            },
        }
    }
}

/// 隔离目录在输出目录下的名字（12 号票：含坏页的卷输出到隔离目录）。
///
/// 名字用 ASCII：输出常常要经 MTP 或 FAT 搬到阅读器上，目录名少一分编码上的赌注是一分。
/// 下划线前缀买两件事——它不至于撞上一个真叫这个名字的卷，列目录时也排在最前面。
const ISOLATED_DIRECTORY: &str = "_isolated";

/// 处理一个卷：分析环节解码到画质分，写出环节量化到写出，非图片成员原样搬过去。
///
/// 两遍之间隔着缓存（ADR 0005：解码一次，缓存缩放后的图）。写出环节的输入是分析环节存下的参照，
/// 源页因此只被解码一次——`VolumeReport::decodes` 是这条不变量看得见的形式。
///
/// 彩页在彩色 profile 下不走这条路：它在分析环节就缩放并编好，绕开缓存、画质分与汇总
/// （ADR 0005 决定第 4 条，见 [`first_pass`]）。写出环节只把它按阅读顺序写出去。
///
/// dry-run 走同一条路，只是不建输出容器，写出环节也就没有可写的地方。
///
/// 两遍之前还有一道**幂等**：上一趟的输出还在、依据一项没变，这一卷就整个不做
/// （见 [`volume_fingerprint`] 与 [`compare_with_the_prior_output`]）。dry-run 也走这一道——
/// 它预告的是照做时会发生的事，而照做时会发生的正是「跳过」（spec 的 story 6、story 8）。
///
/// 要摊开的卷（`.rar` / `.7z`）在幂等之前还有一个环节：**摊开**——重开这一卷时整卷解到
/// 临时目录（ADR 0015 决定第 3 条，见 `source::open`），此后按目录卷走。幂等命中的卷也走它：
/// 查重要读源字节，源字节要先摊开。
///
/// **依据按这一趟的作用域只有一种**（two-pass-rework/15；`CONTEXT.md` 的《源哈希》）：
/// 一页的字节由全卷定（`--envelope` 那条路）就按卷——全卷一个源哈希，整卷跳或整卷重做；
/// 只取决于它自己（默认路径）就按页——每一页自己一份，**卷不齐时按页**（two-pass-rework/14；
/// ADR 0018 决定第 4 条）：页级依据对得上的页**留下**——不读、不解、不判、不编，写出环节从
/// 上一趟的输出里原样搬过来；对不上的页重做。卷仍是去处、撞名、透传文件的单位：留下的与
/// 重做的按阅读顺序一起写进同一个容器，收尾照旧整个换掉（见 `crate::sink`）。
///
/// **卷的去处到分析环节走完才定得下来**（12 号票）：有坏页的卷整个进隔离目录，
/// 而哪一页失败要解过才知道。输出容器因此在分析环节之后才建——写出全在写出环节，
/// 早建一步只会让隔离的卷在干净的去处留下一个空壳。
///
/// **隔离的卷不被幂等跳过**：跳过只认干净的那个去处（见 [`compare_with_the_prior_output`]）。
/// 这是有意的——那不是一份做完了的输出，而失败清单每一趟都要重新给得出来（spec 的 story 26）。
/// 代价是有坏页的卷每趟都重做一遍，直到坏页被修好——按页那一支上重做的只是与干净去处里
/// 那一份对不上的页，坏页修好之后没变的页照样留下。
///
/// `probes` 是这一趟共用的那份硬盘类型探测（ADR 0009 决定第 2 条，见 `medium`）。这一卷问它
/// 一次，答案变成一份[读取计划](IoPlan)：这一卷读几条、为什么是这个数，报告照它说。
/// **问在重开这一卷之后**——探的是这一卷此刻真正住的那个路径，而摊开的卷要摊开了
/// 才住得进临时目录里去（见本函数里那一句上的注释）。
///
/// 收的是一份**清点摘要**（见 `survey`）——这一卷的路径与几个数，不是卷本身：
/// 清点数完就把卷放掉了，这里**按那个路径再开一次**。为什么宁可读两遍中央目录也不攥着它，
/// 见 `survey` 的模块文档。重开这一遍落在这一卷的墙钟之内，成员也按重开的这一份重新数
/// （见 [`MemberCounts::of`]）。
///
/// # 立即停止：回 `None`
///
/// **[页边界那个检查点](progress::Events::aborting)问在这几处**（ADR 0013 决定第 2 条）。
/// 凡是**逐个成员**往下走的循环，循环头上都问一次——开工前[摊开一整卷](source::open)
/// 那两遍顺序扫（`source::spread_seven_zip` 与 `spread_rar`，`p4-parking-lot/13`）、
/// 幂等这一道、分析环节、写出环节写页、写出环节搬透传文件；**外加两处不是循环头的**：
/// 摊开开工之前（`source::open`：开卷那一条上就答了立即停止的话，摊开那个环节连开工都不报），
/// 以及本函数里 `source::open` 紧接着那一句——摊开途中按下的那一下要在那里收口，
/// 它交出来的是一份半摊开的卷（见那一句上的注释）。
/// 答立即停止就当场停下，这一卷回的是 `None`。
/// 不逐个数它们，也不在别处复述这个清单：数目会随管线长，而这里是它唯一的出处。
/// 摊开那三处**落在读取那一层**，不在本函数里——「唯一的出处」说的是这张清单，不是这个文件。
///
/// `None` 说的是**那一卷等于没做**：它那格 `partial` 没有收尾、由析构丢掉
/// （见 `crate::sink` 的两个 `Drop`），最终位置上一个字节都没动过，报告里因此
/// 也不该有它的位置。写出环节开始之前立即停止的话连那一格都还没建。
///
/// 每一段停下之后都**再问一次**闩，而不是把「我是被立即停止的」当成返回值一层层传上来：
/// 闩只升不降，再问一次恒得同一个答案（见 [`progress::Events::aborting`]）。
///
/// # 做完再停：停在确认点上
///
/// **接着写出的确认点**在「汇总之后、写出环节之前」，一卷一次（ADR 0012 决定第 2 条）：
/// 答继续就往下做，答做完再停就**停在这儿**。停下来的现场与立即停止不同，两件事都要看清——
///
/// - 回的是 `Ok(Some(report))`，不是 `None`：这一卷**做过事**，判定、逐页结果、缓存用量、
///   解码计数都是真的，只是写出环节一步没走。那正是 dry-run 的效果（spec 的 story 6），
///   而报告本来就是预览要看的那份东西。
/// - **输出一个字节都不写**：输出容器连建都不建，`partial` 因此也没有。
/// - **参照还在缓存里**：这一卷的缓存活到 `run` 走完（ADR 0012 决定第 4 条），
///   会话答继续的那一次由同一趟 `run` 接着做——接着写出不跨调用。
/// - 这个字照样进闩，所以**剩下的卷不必开工**：卷边界那个检查点接着拦下它们。
///
/// 这一处认的是**当场答的那个字**，不是闩——为什么，见
/// [`progress::Events::ask_before_the_second_pass`]。
///
/// # 失败：回 `Err`
///
/// 这一卷做不成就回 `Err`，**整趟不因此停下**（05 号票）：`run` 把它记成一笔
/// [卷转换失败](VolumeFailure)，其余卷照做（见 `run` 的《两种失败分得开》）。
/// 卷根整个不见了（见 [`ensure_the_volume_root_is_still_there`]）、重开这一卷点不开、
/// 撞名、指纹那一道读不出字节、分析环节读不出源、建不出输出容器、透传文件搬不动，
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
    // 这一卷的两个可能去处。哪一个作数要等分析环节走完才知道，另一个则可能留着上一趟的过期副本。
    //
    // 两个都由清点交过来的那条**镜像相对路径**接出来（见 `survey::Surveyed::output_path`）：
    // 输出镜像源的结构，基准点是处理路径的父目录（ADR 0014 决定第 4 条）。
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
    // 这一卷的表：四段各自掐（加固批 11 号票，见 [`VolumeTiming`]）。总的那个数从这里起算，
    // 也就是**在重开这一卷之前**；**再把清点枚举它的那一截加回去**——枚举两遍都是这一卷
    // 真花掉的时间，一遍在这个表里，一遍由清点交过来（见 `survey::Surveyed::enumerating`），
    // 而 `outside_the_segments` 的文档正指着它说「少掉的那一截恰恰是枚举」。
    let started = Instant::now();
    // **开卷时**的累计读数，与 `started` 成一对。这一卷的墙钟要减掉「在确认点上等人」
    // 的那一截（停车场 Q41），而那一截就是这个快照与拼报告时那个读数之差——
    // 累计只升不降，见 `progress::Deliberation`。
    let deliberated_at_open = events.deliberated();
    // 这一卷的墙钟：从重开这一卷（外加清点枚举它的那一截）算到这份报告成型，减去等人的那一截。
    let wall_clock = || {
        let deliberated = events.deliberated().saturating_sub(deliberated_at_open);
        enumerating + started.elapsed().saturating_sub(deliberated)
    };
    let mut timing = VolumeTiming::default();
    // 开卷那一条排在**这一卷的第一件事之前**：往后每一条出口——一卷跑完、卷转换失败、
    // 立即停止——都在它之后，画进度的那一层因此不必分「这一卷开过头没有」两种情形
    // （见 `progress::Event::VolumeFailed`）。它排在下面那道「卷根还在不在」之前
    // 正是为了这个：那一道是这一卷最早的一个 `Err`。
    // 它报得出卷根与步数，靠的正是清点留下的那两样——重开还没发生，这里也不需要它发生。
    events.volume_started(&root, steps);
    // 卷根在清点之后整个消失是**这一卷没做成**，不是「这一卷全是坏页」（05 号票）。
    // 画质分、为什么非得在这里单问一句，见 [`ensure_the_volume_root_is_still_there`]。
    ensure_the_volume_root_is_still_there(&root)?;
    // **按路径再开一次**：清点只数不留，卷在它手上已经放掉了（见 `survey`）。
    // 开在这里而不是更早，是因为上面那两句一个要在最前、一个要抢在真读字节之前。
    //
    // **固实归档就在这一句里摊到临时目录**（ADR 0015 决定第 3 条）——「开工前」指的正是
    // 这个位置：卷根还在的那一道已经过了，而下面每一件要源字节的事都还没开始。
    // 摊不下（磁盘不够）从这里回 `Err`，那是卷转换失败，其余卷照做。
    //
    // **观察者那条回路一并递进去**（`p4-parking-lot/13`）：摊开一整卷要跑很久，
    // 那一段里报得出步、也停得住（见 `source::open`）。
    //
    // **摊开那一段的表一并交进去**（say-and-stop/03）：摊开是一个环节，而它夹在这一句的中间
    // ——成员列齐之后、卷交出来之前——这一层够不着它的两头（见 `source::open` 的《摊开那一段的表》）。
    // 不摊开的卷那一格留着零。
    let mut volume = source::open(&root, events, &mut timing.extraction)?;
    // **页边界那个检查点**，摊开途中按下的那一下在这里收口：`source::open` 交出来的
    // 是一份**半摊开**的卷（成员表齐、临时目录里只有停之前落下的那几个），
    // 底下每一件事都要源字节，一件都不能做。丢掉它连临时目录一起收走（见 `source::Extraction`），
    // 写出环节还没开始、一格 `partial` 都还没建，最终位置纹丝不动。
    if events.aborting() {
        return Ok(None);
    }
    // 摊了多少字节先留一份：报告两处都要它，而其中一处（跳过那一支）会把卷根搬走。
    let extracted = volume.extracted();
    // 成员按**重开的这一份**数，不是清点那一份：报告说的得是真做了的这一卷
    // （见 [`MemberCounts::of`]）。
    let members = MemberCounts::of(&volume, request);
    // 这一卷的输出成员名此刻只预告得出**一对一那一套**：一个源页产出几张要解了像素才知道
    // （有没有中缝，页几何批 04 号票），而这一步在解码之前。撞名因此查两遍——
    // 这一遍拦下与内容无关的那些（`001.jpg` 与 `001.png` 撞在同一个输出上、归档里的同名成员），
    // 真正产出的那批名字等分析环节走完再查一遍。早查这一遍买的是**别白做一整卷**。
    ensure_one_member_per_output(&volume, &one_to_one_targets(&volume))?;
    let source_pages = members.source_pages;

    // **硬盘类型按这一卷此刻真正住的那个路径探**（ADR 0009 决定第 2 条）：按路径探测那条边界
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
    // （见 [`Settles`]）。两样从前在分析环节里备，提到这里是因为幂等那一道也要问后者：
    // **页级依据在这一趟成不成立**，答的是照做那一趟的形态（见 [`Settles::if_processing`]）。
    let candidates = Candidates::new(request)?;
    let settles = Settles::for_this_run(request, &candidates);
    // 上一趟写在干净去处的输出，比对与留下的页都从它来（two-pass-rework/14）。
    let prior_output = request
        .metadata
        .then(|| sink::Written::open(&clean, volume.container))
        .flatten();
    // 源哈希按哪种作用域算、记、比（two-pass-rework/15）：这一页的字节只取决于它自己（分析环节
    // 就编好）就按页，由全卷定就按卷——一处出处，与 13 号票写不写页级依据的是同一句
    // （`CONTEXT.md` 的《源哈希》）。问的是**照做那一趟**的形态，预览要预告的是照做时会发生的事。
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
        let segment = &mut timing.fingerprint;
        timed_pass(events, Pass::Fingerprint, segment, || -> Result<_> {
            let fingerprint = volume_fingerprint(&mut volume, request, &io, by_page, events)?;
            // 立即停止之后**不再问幂等**。不是因为答案会错——下一句就把整卷连同这个答案一起
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
        // 立即停止停在幂等这一道上：写出环节还没开始，一格 `partial` 都还没建，
        // 最终位置纹丝不动（见本函数的《立即停止：回 `None`》）。
        return Ok(None);
    }
    // 三种答案三条路（见 [`Reuse`]）：整卷跳过在这里就收摊；按页那一支带着留下的页与
    // 打开着的上一趟输出往下走（two-pass-rework/14）；整卷重做一页都不留。
    // 往下分析环节只走要重做的那些源页，写出环节按阅读顺序把留下的照搬、重做的写出。
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
                // 分析、写出两个环节一个都不走：四段里有数的只有查重，外加要摊开的卷上的摊开。
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

    // dry-run 没有写出环节，缓存于是只记账不留页：用量照旧预告得出，临时文件一个不建。
    let retention = match request.mode {
        Mode::Process => cache::Retention::Keep,
        Mode::DryRun => cache::Retention::Account,
    };
    // 缓存与那几个计数是计算层唯一共用的东西：缓存要串起来（账本只有一本，参照那个数
    // 就记在它上面），解码与缩放两个数各是一次原子加。贵的那几步——解码、缩放、画质分、压缩
    // ——全在锁外。
    let cache = Mutex::new(cache::PageCache::new(request.cache_budget, retention));
    let counters = ComputeCounters::default();
    // 分析环节产出的是**输出页**：一个源页产出的那几张挨着排，卷内页序就是写出顺序。
    let FirstPass {
        pages: scored,
        settled,
    } = timed_pass(events, Pass::First, &mut timing.first_pass, || {
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
        // 立即停止停在分析环节的页边界上：手上这半份逐页结果连同这一卷一起丢掉。
        // 写出环节还没开始，一格 `partial` 都还没建，最终位置纹丝不动。
        //
        // 它排在下面那条 `debug_assert!` **之前**：半份结果本来就凑不齐预告的张数，
        // 而那条断言问的是「拆分与预告有没有分家」，立即停止不是它要抓的东西。
        return Ok(None);
    }
    // 预告的张数与真产出的张数在这里第一次同时在手上。预告是**上界**（页几何批 04 号票：
    // 一个源页产出几张由内容决定），因此比的是区间而不是等号：下界是一张源页至少出一张，
    // 上界是每张都被切开。越出这个区间说明拆分与预告分了家，而那是一种静默的错——
    // 报告照出，进度条却要么冲过头、要么停在半路。
    let at_most = redo.len() * max_outputs_per_source_page(request);
    debug_assert!(
        (redo.len()..=at_most).contains(&scored.len()),
        "分析环节产出 {} 张，而重做的源页 {} 张、上界 {at_most} 张",
        scored.len(),
        redo.len()
    );

    let (verdicts, verdict) = cost::stage(cost::Stage::Summarize, || {
        summarize_volume(&scored, request, &candidates)
    });
    // 一张灰度页都没重做、却留下了页（只补透传文件、重做的只有彩页）：这一卷的候选仍是从
    // 这条路上来的——留下的页正是上一趟按这条路判的——报告说的就该是这条路，而不是
    // 「一张灰度页都没有」（two-pass-rework/14）。整卷重做的卷这里是 `None`，不动。
    let verdict = verdict.or_else(|| {
        (retained_pages > 0)
            .then(|| walks.verdict_by_itself())
            .flatten()
    });
    // 留下的与重做的按阅读顺序交错成写出环节要写的那一串（two-pass-rework/14）。
    let slots = in_reading_order(&volume, &retained, &scored, &verdicts)?;
    // 真正产出的那批成员名在这里第一次齐了：加了序号的名字可能撞上卷里本来就有的成员
    // （源里同时有 `001.jpg` 与 `001-1.png`），而那一撞要在写出第一个字节之前拦下。
    // 留下的页照样在这一批里：新切出的一张与留下的一张撞名，同样不能静默覆盖。
    ensure_no_two_outputs_collide(&volume, &slots)?;
    // 分析环节提前编好字节的那两条路各自也定了一份档，而字节已经照它编好了
    // （默认那条路与顶死那一条，见 `pipeline::first_pass_verdicts`）。
    // 两份必须逐格相同：报告说的那一档与写出去的那一页，一处出处。
    debug_assert!(
        settled.as_ref().is_none_or(|settled| *settled == verdicts),
        "分析环节编字节用的档与汇总定的档分了家：{settled:?} 对 {verdicts:?}"
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

    // **这一卷的报告拼两次，拼法只有这一处。**一次在下面那个确认点上——交给观察者的就是它
    // （停车场 Q52：不给它，要在那里等人拿主意的调用方屏上画不出任何东西）；
    // 一次在这一卷收摊时。两次之间夹着写出环节，因此逐页那一步是**借着算**的
    // （见 [`OutputPage::to_report`]），差的只有交进来的那份计时。
    // 各拼各的话，屏上那一份与最终报告迟早会分家。
    //
    // 读缓存那两个数要的那把锁**掐在这个闭包里，而且掐在自己那个块里**：拼完就要把这份报告
    // 交给观察者，而观察者可能很久不返回（见 `progress` 的模块文档）。两个数一并读回来，
    // guard 出了那个块就没了，因此走到下面那个确认点时手上已经空了。
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

    // **接着写出的确认点就在这一句上**（ADR 0012 决定第 2 条）：汇总已经做完、写出环节还没开始。
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
            // 答继续：往下做。参照还在缓存里，分析环节不重算——那正是接着写出买的东西。
            Instruction::Continue => true,
            // 答做完再停：**停在这儿**。那一卷等于走了一次预览，输出一个字节都不写、报告照出
            // （见本函数的《做完再停：停在确认点上》）。
            Instruction::Finish => false,
            // 答立即停止：这一卷等于没做，与页边界上按下它一个待遇（见《立即停止：回 `None`》）。
            // 一格 `partial` 都还没建，最终位置纹丝不动。
            Instruction::Abort => return Ok(None),
        }
    } else {
        // dry-run 一个文件都不落盘，写出环节无从谈起，也就没有「还做不做」可问：
        // 确认点连报都不报（spec 的 story 6）。
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
                // 透传文件也是写出环节写出的成员，页边界那个检查点照样在循环头上。
                if events.aborting() {
                    break;
                }
                let bytes = volume.reader.read(extra)?;
                sink.write_extra(&extra.relative, &bytes)?;
                events.step();
            }
            if events.aborting() {
                // **立即停止：不收尾。** `sink` 在这里走出作用域，它那格 `partial` 由析构丢掉
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

/// 这一卷每个源页**当它一张都不切时**的输出成员名：外层按源页序，内层按阅读顺序。
///
/// 这一份是碰像素之前唯一给得出来的名单：一个源页产出几张由内容决定（页几何批 04 号票），
/// 而这一步在解码之前。它只喂开工前那道撞名校验——拦下与内容无关的那些
/// （`001.jpg` 与 `001.png` 撞在同一个输出上、归档里的同名成员），
/// 买的是**别白做一整卷**。真正产出的那批名字等分析环节走完再查一遍
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
/// 卷根整个消失走不到别的任何一条卷转换失败上。读不出字节的成员在幂等那一道被记成
/// 「读不出来」照样喂进哈希（见 [`volume_fingerprint`]），在分析环节里变成**坏页**
/// （p0 的 12 号票），于是整卷成了一沓占位白页进隔离目录：报告说得出「这一卷全是坏页」，
/// 却不说它压根不在了，退出码也停在 `2`。那不是一卷做出来了。
///
/// # 判定依据是**卷根还在不在**，不是「成了几页」
///
/// 「一页都没成」与「卷根不在」是两件事，落在两个退出码上：
///
/// - 卷根在，而里面每一页都读不出来、解不出来 → 页级失败照旧，整卷进隔离目录（`2`）。
///   那一卷**做出来了**：页序、卷内统一尺寸、透传文件都在，坏的是内容
///   （`tests/isolation.rs` 的 `a_volume_whose_every_page_fails_still_comes_out_whole`）。
/// - 卷根不在 → 卷转换失败（`3`），报告指得出是哪一卷、说得出为什么。
///
/// 拿「成功页数为 0」当画质分会把前一种一起收走，而那一种正是 p0 的 12 号票定死的东西：
/// 一页读不出来不毁掉整卷。本票改的是它上面一层，不是推翻它。
///
/// # 两种容器形态都问
///
/// 问的是**读取层真会去走的那条路**，而两种形态走的是同一条：卷在清点里已经放掉，
/// 轮到它时按路径重开（见 `survey`）——路径不在，[`source::open`] 第一步就走不下去。
///
/// 归档卷从前不问：它的字节从清点时就打开、此后一直握着的那个句柄里出，卷根被删掉
/// 也照读不误，报「没做成」是撒谎。清点不再攥着那个句柄，这条豁免跟着没了
/// （`volume-discovery/01`）。
///
/// 那一版里 Q50 那个缺陷因此只在目录卷这一侧；现在两侧同形，一句话说得完两种。
///
/// # 问在这里，也只问这一次
///
/// 问在**这一卷的第一件事上**：往下每一步都要读它的字节，卷根不在的话那几步全是白工——
/// 重开一次、幂等把整卷哈希一遍、分析环节逐页解一遍、写出环节再写一整卷白页出去。
///
/// 重开那一次**自己也会失败**，因此这一问买的不是「早一步发现」，是**那句话**：
/// 不问的话报出来的是 `source::open` 的「X 不存在」，说不出「它是在清点之后不见的、
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
            "{} 在清点之后不见了：这一卷没做成——不是它里面的页坏了",
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

/// 分析环节**真产出**的那批成员名互不冲突（页几何批 04 号票）。
///
/// 开工前那一遍（[`ensure_one_member_per_output`]）只查得了一对一那套名字：切开之后
/// 加的那个序号可能撞上卷里本来就有的成员——源里同时有 `001.jpg` 与 `001-1.png`，
/// 前者被切成两张，`001-1.png` 就有两个主人。那一撞要在**写出第一个字节之前**拦下，
/// 而这里正是两批名字第一次同时在手上的地方（写出环节还没开始，输出容器还没建）。
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
/// （画质分见 [`normalises_an_extension`]）。
///
/// **查的是发现出来的那些卷**，不是点名的那几个路径：点名的是「在哪里找」，
/// 不是「找到什么」。这一道因此排在清点之后（见 `run`），撞车仍在写出第一个字节之前说。
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
        said.push_str("分批处理，每批给一个自己的输出目录。");
    }
    if by_extension {
        said.push_str(&format!(
            "\n上面有一对只差扩展名：归档卷的输出扩展名一律归一成 .{}，源那一头叫什么\
             扩展名都不带过来，两份包因此指着同一个去处。这一对换输出目录分不开\
             ——卷名本来就相同——只点名其中一份。",
            source::OUTPUT_ARCHIVE_EXTENSION
        ));
    }
    bail!(said)
}

/// 这一组撞在一起的卷里，有没有**扩展名归一**的份。
///
/// 判定依据是**文件名不同**。卷名撞车的两个源文件名必然相同（`甲部/第1话` 与 `乙部/第1话`
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

/// 输出目录写不写得进，**开工前探一次**（06 号票）。
///
/// `--out` 指到一个建不出来的路径，错在这一趟的**参数**上，换一个卷不会变好——
/// 按 `CONTEXT.md` 的《失败》那就是**拒绝开始**：整趟当场停，一页都不做，那句话只说一次。
/// 不问的话它由 [`Sink::create`] 在每一卷里各撞一次（停车场 Q51）：报出来的东西没骗人，
/// 但本该说一次的话说了 N 遍，而那时的退出码是「有卷没做成」（`3`），
/// 含义弱于本该给的那个 `1`。
///
/// 出来的错误**不戴 [`Refusal`]**：那件外套认的是从 [`process_volume`] 里出来的错误——
/// 一卷做不成与整趟别做了在那条路上长得一样。开工前这几道检查的错误由 `run` 直接返回，
/// 调用方拿到 `Err` 本身就是「这一趟没做成」（退出码 `1`），没有第二种可能要分辨。
///
/// # 只答「输出目录写得进吗」
///
/// 拦下的只有**输出目录**这一个、对每一卷都一样的事实。逐卷现建输出容器那条路一个字没改：
/// 某一卷的去处被占、单卷权限不同都到得了那里，那些是真的「这一卷做不成」，
/// 仍走卷转换失败（`3`），其余卷照做。
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
/// 反过来，预览要是答不出「你这个 `--out` 根本用不了」，那份报告就在撒谎：拒绝开始说的是
/// **参数**，而两种模式的参数是同一份，另外几种拒绝也都在预览里照样咬人
/// （`CONTEXT.md` 的《会话》：预览与转换是同一个 `run`，区别只在 mode）。
///
/// # 它拦不住什么
///
/// 探得过而真写时才坏的那些——盘满，或者探过之后输出目录被删、权限被改——仍旧落回
/// **卷转换失败**（`3`），与 [`ensure_the_volume_root_is_still_there`] 那一处的竞态同一个形状：
/// 一次探测拦不住时间。这一道要的不是「此后一定写得出去」，
/// 是**开工前就知道写不出去的那一种别再一卷一卷地撞**。
///
/// 同一条时间上还有两笔**认下的代价**，都窄，也都没有便宜的躲法。一是「哪几级要现建」是
/// 探之前记下的：另有人恰在那之后把同名目录建起来，收拾那一步会把它删掉——删得掉的至多是
/// 一个空壳（`remove_dir` 只删空目录），而两趟 tonefit 同时探同一个输出目录时，
/// 被删的那一趟随后自己再建一遍。二是进程正好在试写与删探针之间被杀：
/// 那个点开头的探针文件会留在输出目录里。躲开两者要一份日志或一把跨进程的锁，
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

    // 两句交代里都不再提一遍路径：它们只被下面那句 `bail!` 收走，而那一句已经把输出目录
    // 指名道姓说过了。再提一遍就是同一个路径在一行里出现两次。
    let probed = std::fs::create_dir_all(output_root)
        .context("建出这个目录")
        .and_then(|()| {
            // 探针名带着进程号：两趟 tonefit 同时探同一个输出目录时各删各的那一个，
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
            "输出目录 {} 写不进去：{error:#}。             这一趟的每一卷都要写到它下面，换一个卷不会变好——一页都没做，请换一个 --out",
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
    //! 一个源页产出**两张**输出页时，装配这一层还跟不跟得上（页几何批 03 号票）：进度步数与撞名。
    //! 各个环节自己那几条在 `pipeline` 的用例模块里；为什么摆在库内而不在 `tests/`，
    //! 那个用例模块的文档说了，判据是同一句。

    use super::*;

    /// 一份最小的请求。各用例只改自己那一处。
    ///
    /// `pub(crate)` 是给别的模块里那几条用例的：`survey` 那几条只改点名的卷与输出目录，
    /// `pipeline` 与 `interlock` 那几条各改自己那一处。
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

    /// 写出那一段按**输出**成员数，读那两段按源那一侧（页几何批 03 号票的进度步数）。
    ///
    /// 分得开才要紧：读源与解源页都发生在切开之前，只有写出那一段跟着切完的张数走。
    /// 混成一个数的话，切开的卷进度条会在写出环节里走过头或者停下不动。
    ///
    /// **摊开那一段**（`p4-parking-lot/13`）在末尾单问一次：它与源那一侧数的是同一批成员，
    /// 却由格式定在不在，因此拿一个不摊开的卷与一个摊开的卷对着看。
    #[test]
    fn the_write_segment_counts_output_pages_and_the_read_segments_count_source_pages() {
        // 三张源页切成五张输出页，外加一个透传文件：幂等读 3+1、分析环节走 3、写出环节写 5+1。
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
        // dry-run 没有写出那一段，切成几张都不改变步数。
        let dry = Request {
            mode: Mode::DryRun,
            ..request()
        };
        assert_eq!(volume_steps(split, &dry), 4 + 3);
        // `--no-metadata` 关掉幂等那一段，写出那一段照旧按输出算。
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
}
