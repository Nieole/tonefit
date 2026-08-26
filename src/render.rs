//! 界面文案：终端上印出来的那一套措辞，命令行与会话共用。
//!
//! 它在**界面层**，不进库：措辞是给人读的，库那一侧只出数据。位置在二进制 crate 内——
//! `lib.rs` 顶上那张模块清单之外的 `src/*.rs` 都是这一侧的。
//!
//! 大头是把 [`Report`] 渲染成文字，分四段，调用方各取所需：[`header`] 一趟只出一次，
//! [`volume`] 与 [`pages`] 逐卷出，[`tail`] 收在末尾。命令行攒完在最后把四段一次性
//! 拼起来（[`report`]）；会话攒到哪儿画到哪儿——卷级事件带着 `VolumeReport`，
//! 那一卷跑完就画得出它那一段（ADR 0011）。卷级与逐页分成两个函数，
//! 是因为会话的报告区默认只给卷级，展开才逐页。
//!
//! 两边拿的是同一份数据，措辞因此只有一套。同一个理由把[标定图那几行](calibration_note)
//! 也收在这里：它不从报告来，但它同样是界面文案，会话按键出图时印的也是它。

use std::path::Path;

use tonefit::{
    CandidateScore, Mode, PageBranch, PageColor, PageReport, Profile, Report, VolumeReport,
    VolumeVerdict, aggregation,
};

/// 整份报告：命令行跑完在最后一次性渲染出来的就是它。
///
/// 四段按顺序拼起来，中间不加任何东西——会话逐段画出来的与这里拼出来的逐字节相同。
pub fn report(report: &Report, mode: Mode) -> String {
    let mut text = header(report, mode);
    for volume in &report.volumes {
        text.push_str(&self::volume(volume));
        text.push_str(&self::pages(volume));
    }
    text.push_str(&tail(report));
    text
}

/// 抬头：这批输出给哪台设备、判据是怎么聚合出来的，以及这一趟写不写盘。
///
/// 一趟只出一次。它吃的是整份报告而不是单独一个 profile——报告是**逐卷攒出来的**
/// （ADR 0011），攒到一半的那一份同样答得出这三件事。
pub fn header(report: &Report, mode: Mode) -> String {
    let mut text = format!("profile {}\n", report.profile);
    // 逐页那些「判据 …」的数都是这套取法收出来的，而取法里的 K 还没标定。
    // 它与阈值同一个待遇：数摆出来，没标定这件事跟着摆出来（ADR 0002 决定第 3 条）。
    // 它自成一行、不接在 profile 后面——判据聚合眼下对所有 profile 都一样，不是这台设备的事。
    // 行首是「判据聚合」而不是「判据」：逐页那一行的「判据」说的是**量**，两者不许同名。
    text.push_str(&format!("判据聚合 {}\n", aggregation()));
    if mode == Mode::DryRun {
        text.push_str("dry-run：只算不写，下面的路径都还没落盘\n");
    }
    text
}

/// 一个卷的**卷级**那几行：去处与页数、过期副本、判定、这一趟怎么读的、缓存用量。
///
/// 逐页那些行不在里面，它们在 [`pages`]。分成两个函数是给会话用的：报告区默认只给卷级，
/// 展开某一卷才逐页（p1 spec 的《会话：布局与交互》）。命令行两段都要，接连拼上去
/// 就是从前那一整段。
pub fn volume(volume: &VolumeReport) -> String {
    let mut text = format!(
        "{} → {}（{} 页{}）\n",
        volume.volume.display(),
        volume.output.display(),
        volume.page_count(),
        color_page_note(volume)
    );
    text.push_str(&superseded_line(volume));
    text.push_str(&verdict_lines(volume));
    // 这一卷是怎么读的（13 号票）。它排在跳过那一支**之前**：幂等命中的卷同样把整卷的字节
    // 读了一遍，读法与做事的那一趟是同一个，而「跳过一卷为什么也要等这么久」正问在这里。
    text.push_str(&format!("  {}\n", volume.io));
    // 跳过的卷什么都没做：缓存用量与逐页结果无从谈起，`verdict_lines` 那一行已经说完了。
    if volume.skipped() {
        return text;
    }
    // 卷成为不可分割的处理单元，峰值内存随卷大小走（ADR 0005）：这一行是那条代价的现场。
    text.push_str(&format!("  缓存 {}\n", volume.cache));
    text
}

/// 一个卷的逐页那些行：每页两行，一行几何、一行判定。
///
/// 跳过的卷一行都不出：那一趟根本没算过逐页结果，摆出任何一项都是编的。
/// 这里问的是 [`VolumeReport::skipped`]——跳过在那个结构上由两处一起体现，
/// 而认哪一处只许有一个出处（见 `VolumeReport::skipped` 的文档）。
/// 拆分之前这道守卫与 [`volume`] 里那道是同一句 `continue`。
pub fn pages(volume: &VolumeReport) -> String {
    if volume.skipped() {
        return String::new();
    }
    let mut text = String::new();
    for page in &volume.pages {
        text.push_str(&format!(
            "  {}  {}  {}\n",
            page.size,
            scaling_note(page),
            page.output.display()
        ));
        text.push_str(&format!("    {}\n", page_line(page)));
    }
    text
}

/// 末尾那两小结：部分救回与隔离。各自一页都没有就一个字都不说。
///
/// 它们要看完整趟才给得出来，因此不进 [`volume`]：那两行数的是**这一趟**有几卷几页，
/// 而不是这一卷。
pub fn tail(report: &Report) -> String {
    let mut text = salvage_tail(report);
    text.push_str(&isolation_tail(report));
    text
}

/// 隔离那一小结，摆在整份报告的末尾。
///
/// 逐页那几行已经把每一个失败页与原因说过一遍了；这一行是给长任务备的：几十卷跑下来，
/// 失败页早滚出屏幕了，而「这一趟到底有没有出事」得有一个不用往回翻的答案。
/// 退出码说的是同一件事（见 `crate::exit_code`），只是那一个给脚本读、这一行给人读。
/// 一卷都没被隔离就一个字都不说。
fn isolation_tail(report: &Report) -> String {
    let volumes = report
        .volumes
        .iter()
        .filter(|volume| volume.isolated())
        .count();
    if volumes == 0 {
        return String::new();
    }
    format!(
        "隔离 {volumes} 卷 · 失败 {} 页：失败页以卷内统一尺寸留白占位，原因逐条列在上面\n",
        report.failures().count()
    )
}

/// 部分救回那一小结，与隔离那一小结并排摆在报告末尾（04 号票）。
///
/// 它比隔离那一行更需要这个位置：含失败页的卷有退出码替它喊，也有一个隔离目录摆在那儿；
/// 部分救回页两样都没有——卷照常落在干净的去处，退出码是 0，而源文件确实不全。
/// 几十卷跑下来，逐页那几行早滚出屏幕了，不在末尾说一句就等于没说。
///
/// 这一行只报数，不重复「它们没参与卷级的哪两件事」——那句话在卷级那一行上，
/// 而这一行与它出现在同一份报告里（见 [`salvaged_line`]）。
///
/// 一页都没有就一个字都不说，与隔离那一行同一条规矩。
fn salvage_tail(report: &Report) -> String {
    let pages = report.salvaged().count();
    if pages == 0 {
        return String::new();
    }
    let volumes = report
        .volumes
        .iter()
        .filter(|volume| volume.salvaged().next().is_some())
        .count();
    format!("部分救回 {volumes} 卷 · {pages} 页：源文件不全，缺的那一段留成纸白\n")
}

/// 过期副本那一行（12 号票）。
///
/// 卷的去处随「有没有失败页」在干净目录与隔离目录之间跳，而这一趟写不到的那一份不会被覆盖、
/// 也不会被删。它可能是**一整卷白页**的占位输出——摆在文件管理器里与一本正经的书没有分别。
/// 报告因此要指名道姓地说出它在哪儿，删不删由用户定。
///
/// 这一行排在卷级各行之前：它说的不是这一趟做了什么，而是上一趟留下了什么。
fn superseded_line(volume: &VolumeReport) -> String {
    match &volume.superseded {
        Some(path) => format!(
            "  过期副本 {}：上一趟写在那儿，这一趟没有覆盖它。\
             那一份当初若是被隔离过的，它整卷都是白页——删不删由你\n",
            path.display()
        ),
        None => String::new(),
    }
}

/// 卷级那一段里说部分救回的那一行，排在隔离那一行之后（04 号票）。
///
/// 隔离那一行说的是「这一卷有页根本没出来」，这一行说的是「有页出来了，但不全」。
/// 两句分开，因为后果不同：前者整卷换了去处，后者没有——这一卷仍在干净的去处，
/// 而卷级的档是在**没有**这几页的情况下定出来的，那正是这一行要交代的事。
fn salvaged_line(volume: &VolumeReport) -> String {
    let pages = volume.salvaged().count();
    if pages == 0 {
        return String::new();
    }
    format!(
        "  部分救回 {pages} 页：整解失败，按文件头的尺寸救回了一段，缺的那一段留成纸白。\
         它们不参与卷级上包络，各自单独定档。几何门照旧问它们——那是文件头里的真尺寸；\
         门在哪一页上也不成立，那一页就改按门那一条来（见上）\n"
    )
}

/// 一页那一行里说缩放的那一小截。
///
/// 失败页没有缩放可说——它没被缩放过（ADR 0001 那三个数一个都不成立）。
/// 那一格于是改说它的尺寸是从哪来的：卷内统一，不是它自己的。
fn scaling_note(page: &PageReport) -> String {
    match page.scaling() {
        Some(scaling) => scaling.to_string(),
        None => "失败页 · 卷内统一尺寸留白".to_owned(),
    }
}

/// 幂等命中而跳过的卷那一行。
///
/// 「跳过」本身不够——用户要能分清「这一卷没变」与「工具没做事」。四项依据点名摆出来，
/// 改了其中哪一项会让它重做，一眼看得见（spec 的 story 8、story 9）。
const SKIPPED_LINE: &str =
    "  跳过 幂等命中：工具版本、profile、参数、源均未变，上一趟的输出还在，这一卷一页都没有重做\n";

/// 卷那一行里说彩页有几张的那一小截。
///
/// 一张都没有就不说——绝大多数卷是这个样子（见 measurements 的《B 类素材普查》：97% 近灰度），
/// 每卷都挂一句「彩页 0 页」只是噪声。数的是**彩页**，与它走了哪条分支无关。
fn color_page_note(volume: &VolumeReport) -> String {
    let count = volume
        .pages
        .iter()
        .filter(|page| page.color() == Some(PageColor::Color))
        .count();
    if count == 0 {
        String::new()
    } else {
        format!("，其中彩页 {count} 页")
    }
}

/// 卷级那几行里说判定的那一段：几何门的判定结果，加上这一卷的候选从哪来。
///
/// 「这卷为什么是这个候选」要有一个指得出驱动页的答案（ADR 0006），这几行就是它。
/// 上包络不在场时说清是为什么不在场——那正是翻页跳变回来的时候，报告不能看起来还是一样。
fn verdict_lines(volume: &VolumeReport) -> String {
    // 一页都没有的卷只装着透传文件，没有候选可判，几何门也就无从谈起。
    let Some(verdict) = &volume.verdict else {
        return String::new();
    };
    // 跳过的卷同样没有几何门可说——它一页都没算。这一支要排在 `gate_line` 之前：
    // 那里读的 `volume.gate` 只有算过的卷才有。
    if volume.skipped() {
        return SKIPPED_LINE.to_owned();
    }
    let mut text = isolated_line(volume);
    text.push_str(&salvaged_line(volume));
    text.push_str(&gate_line(volume, verdict));
    text.push_str(&match verdict {
        VolumeVerdict::Envelope(envelope) => format!(
            "  卷级 {envelope}\n    驱动页 {}\n",
            volume.pages[envelope.driver].source.display()
        ),
        VolumeVerdict::Override(candidate) => format!(
            "  卷级 判定 {candidate}（覆盖项裁到只剩一个候选）：判定被顶掉，卷级基准档无从谈起\n"
        ),
        VolumeVerdict::PerPage => {
            "  卷级 无（--per-page）：上包络与迟滞关着，候选逐页最优，翻页处会换档\n".to_owned()
        }
        // 上面那一支已经把跳过的卷送走了。
        VolumeVerdict::Skipped { .. } => String::new(),
    });
    text
}

/// 被隔离的卷那一行，排在卷级各行之首（12 号票：含失败页的卷被标记）。
///
/// 卷那一行里的去处已经指着隔离目录了，但那要用户认得出那个目录名才读得懂。
/// 这一行把话说完：几页失败、这一卷因此去了哪儿、坏页在输出里是什么样子。
/// 后面几行照常——隔离的卷是**处理过**的卷，几何门、卷级判定、逐页结果一样不少。
fn isolated_line(volume: &VolumeReport) -> String {
    let failed = volume.failures().count();
    if failed == 0 {
        return String::new();
    }
    format!(
        "  隔离 {failed} 页失败：本卷整卷写到隔离目录 {}，\
         失败页以卷内统一尺寸留白占位，页序不断\n",
        volume.output.display()
    )
}

/// 几何门那一段：门的**判定范围**、范围里有几页不成立，加上本卷最终抖不抖。
///
/// 三件事写在一起，因为只有并排才解释得了对方。门逐页判（ADR 0007 决定第 1 条），
/// 「成立」这句话因此得连着范围一起读——一卷全是彩页时门同样成立，而那是「无人可关」，
/// 不是「每一页都贴住了面板」。本卷那个抖动模式同理：门在主体那一组上开着时它才是判据选的，
/// 主体一页都不成立时它只是被关掉的结果。
///
/// **被排除的页要指得出来**，与上包络指出驱动页同一个做法：不指名，用户就无从判断
/// 这一卷该不该换个 profile。逐页那几行各自标着理由（`几何门不成立，本页不抖动`），
/// 这里只给个抓手——页数多起来时全列一遍只会把卷级那几行淹掉。
fn gate_line(volume: &VolumeReport, verdict: &VolumeVerdict) -> String {
    let judged = volume.judged_by_the_gate().count();
    let broken: Vec<&PageReport> = volume.outside_the_gate().collect();
    // `--per-page` 一开就没有卷级的抖动模式：它跟着位深一起逐页可变。
    let dither = verdict
        .dither()
        .map_or_else(|| "逐页".to_owned(), |dither| dither.to_string());
    let mut text = format!(
        "  几何门 判定范围 灰度页 {judged} 页 · 不成立 {} 页 · 本卷 {dither}\n",
        broken.len()
    );
    if broken.is_empty() {
        return text;
    }
    if broken.len() == judged {
        // 一页成立的都没有：没有别人可护，这些页自己就是主体，卷级那一档由它们定出
        // ——那一档必然不抖（ADR 0007 决定第 5 条）。
        text.push_str(
            "    范围内一页都不成立：每一页源都比目标小，按不放大原样输出，\
             阅读器还要再缩一次。没有别人可护，卷级基准档由它们自己定出，抖动因此整卷关闭\n",
        );
    } else {
        text.push_str(&format!("    不成立：{}\n", first_few_names(&broken)));
        text.push_str(
            "    这几页源比目标小，原样输出，阅读器还要再缩一次：它们不进卷级上包络，\
             抖动单独关掉，位深仍跟着卷级基准档、不低于它\n",
        );
    }
    // 同一道门也撑着面板灰阶那道硬上界：像素与灰阶不再对齐，「多出来的级到不了眼睛」
    // 就不再成立。ADR 0003 说了不得沿用，也说了该用哪个集合尚未测量——P0 仍照它裁，
    // 报告因此得把这句话说出来，而不是让它烂在一句注释里。
    text.push_str(
        "    面板灰阶上界的依据在这几页上随门一起失效，\
         P0 仍按它裁候选位深（ADR 0003：该用哪个集合尚未测量）\n",
    );
    text
}

/// 头几页的名字排成一句，剩下的报个数收口。
///
/// 上界取三：这一句是给人抓手用的，不是清单——真要逐页看，逐页那几行一页不落地列着。
fn first_few_names(pages: &[&PageReport]) -> String {
    const SHOWN: usize = 3;
    let listed: Vec<String> = pages
        .iter()
        .take(SHOWN)
        .map(|page| page.source.display().to_string())
        .collect();
    match pages.len().checked_sub(SHOWN) {
        Some(rest) if rest > 0 => format!("{}，另有 {rest} 页", listed.join("、")),
        _ => listed.join("、"),
    }
}

/// 一页那一行：它走的分支，以及那条分支得出的结果。
///
/// 灰度路径给的是判定与判据。判据是量、阈值是界：判定从两者的比较来，因此两者都得摆在
/// 同一行上，判定才是可解释的（spec 的 story 7）。阈值在头一行的 profile 里，
/// 它对整份报告只有一个。
///
/// 彩色分支上没有判定可说，那一行说的是它为什么没有：那条路径只缩放（ADR 0005 决定第 4 条）。
/// 彩页转灰走的是灰度路径，行首标出来——不标，用户就看不出这一档位深是替一张彩页定的，
/// 也看不出这台设备为什么没留住颜色。
///
/// 失败页那一行说的是**原因**（spec 的 story 26）：报告要让用户知道该去修哪几张。
/// 原因是由内到外的整条错误链，最外一环指得出是哪一页、卡在哪一步。
fn page_line(page: &PageReport) -> String {
    let Some(branch) = page.branch() else {
        return format!("失败 {}", page.failure().expect("没有分支的页必是失败页"));
    };
    // 部分救回页标在行首（04 号票）：它有判定、有判据、有自己的尺寸，逐页那一行因此
    // 与一张完好页长得一模一样，而它的判据是在一页大半留白的图上求出来的。
    let salvaged = match page.salvage() {
        Some(salvage) => format!("{salvage} · "),
        None => String::new(),
    };
    match branch {
        PageBranch::Gray {
            scores, verdict, ..
        } => format!(
            "{salvaged}{}判定 {}（{}）  判据 {}",
            if page.color() == Some(PageColor::Color) {
                "彩页转灰 · "
            } else {
                ""
            },
            verdict.candidate,
            verdict.reason,
            score_line(scores)
        ),
        PageBranch::Color => {
            format!("{salvaged}彩页 · 彩色分支：只缩放，不量化，不进灰度缓存也不进卷级上包络")
        }
    }
}

/// 一页各候选的判据值排成一行，候选由小到大。
fn score_line(scores: &[CandidateScore]) -> String {
    scores
        .iter()
        .map(|scored| format!("{} {}", scored.candidate, scored.score))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// 标定图写出去之后印的那几行：图在哪儿，以及**此刻**要做对的那一件事。
///
/// 它不从报告来，却与报告同属界面文案：会话里按键出图印的也是它（会话批的 13 号票），
/// 措辞因此和别处一样只留一套。
///
/// 只说这一件。怎么数、数出来的数是什么意思，图内印着，`--help` 里也写着——
/// 同一套说法在终端上再抄一遍，改的时候就得记着改三处。
/// 留下的那一条之所以在这里，是因为它在别处已经来不及：图一旦被缩着显示过，
/// 数出来的就不是这块面板了，而用户正是在这一刻决定怎么打开它。
///
/// 面板规格不重复——头一行的 `profile` 里已经有了。
pub fn calibration_note(profile: &Profile, out: &Path) -> String {
    format!(
        "profile {profile}\n\
         标定图 {}\n  \
         拷进设备，以原尺寸打开：关掉缩放，也关掉适配屏幕——\
         图被缩过一次，数出来的就不是这块面板了\n  \
         怎么数印在图内（大写英文，中文字模装不进一张位图）；\
         完整说法见 tonefit calibrate --help\n",
        out.display(),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    // 退出码不是渲染的事，用例却要问它：「报告说得出隔离」与「退出码分得开隔离」是同一条
    // 验收的两半（12 号票），拆到两个用例就得把那一大摊报告再拼一遍。
    // 够得着命令行那一侧的只有这个 `#[cfg(test)]` 块：上面那些渲染函数一个符号都不碰它。
    use crate::{ISOLATED_EXIT, SUCCESS_EXIT, exit_code};
    use tonefit::{
        BitDepth, CacheBudget, CacheUsage, Candidate, ChosenBy, Dither, Envelope, GeometryGate,
        GrayImage, IoPlan, Medium, PageOutcome, Processed, Reason, Reference, Salvage, Scaling,
        Size, Verdict,
    };

    /// 一份卷级上包络。渲染这一侧只关心它有没有被说出来，一页的卷取那一页作驱动页。
    fn envelope(base: Candidate) -> Envelope {
        Envelope {
            base,
            driver: 0,
            body_pages: 1,
            outlier_pages: 0,
            raised_pages: 0,
        }
    }

    /// 一份读取计划。渲染这一侧只关心它有没有被说出来，取「探到固态盘、并发读」那一种。
    fn io_plan() -> IoPlan {
        IoPlan {
            medium: Medium::Solid,
            readers: 8,
            chosen_by: ChosenBy::Probe,
        }
    }

    /// 一份缓存用量。渲染这一侧只关心它有没有被说出来，数值取整好读的。
    fn cache_usage() -> CacheUsage {
        CacheUsage {
            budget: CacheBudget::default(),
            pages: 1,
            raw: 4 * 1024 * 1024,
            stored: 1024 * 1024,
            resident: 1024 * 1024,
            spilled: 0,
        }
    }

    /// B 类中位页缩到基准面板：总缩放比 1.219，不触发预缩（见 measurements 的《B 类素材普查》）。
    fn typical_scaling() -> Scaling {
        Scaling::plan(Size::new(1441, 2048), Size::new(1182, 1680))
    }

    /// 一份一页的报告。各用例只改自己那一处，别处照抄默认。
    ///
    /// 几何门不在参数里：它跟着那一页走（`PageBranch::Gray` 的 `gate`），
    /// 卷级那一段是从页数出来的（06 号票）。
    fn one_page_report(profile: Profile, verdict: VolumeVerdict, page: PageReport) -> Report {
        Report {
            profile,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                verdict: Some(verdict),
                cache: cache_usage(),
                io: io_plan(),
                decodes: 1,
                pages: vec![page],
            }],
        }
    }

    #[test]
    fn a_dry_run_says_nothing_was_written_and_gives_the_metric_for_every_candidate() {
        // 判据值从公开 seam 上真算一个：报告要显示的就是它。
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128]));
        let one_bit_dithered = Candidate::new(BitDepth::One, Dither::FloydSteinberg);
        let score = tonefit::score(
            &reference,
            &tonefit::quantize(reference.image(), one_bit_dithered),
        );
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(one_bit_dithered)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                outcome: PageOutcome::Whole(Processed {
                    scaling: typical_scaling(),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore {
                            candidate: one_bit_dithered,
                            score,
                        }],
                        verdict: Verdict {
                            candidate: one_bit_dithered,
                            reason: Reason::LowestWithinThreshold,
                        },
                    },
                }),
            },
        );

        let text = super::report(&report, Mode::DryRun);

        assert!(text.contains("dry-run"), "{text}");
        assert!(text.contains("还没落盘"), "{text}");
        // 比值 < 2 的一页：报告要说出它没预缩，残差段就是全部。
        assert!(text.contains("缩放比 1.219 · 未预缩"), "{text}");
        assert!(text.contains(&format!("判据 1bit+FS {score}")), "{text}");
        // dry-run 也给判定：预告的就是照做时会写出的那一个候选。
        assert!(text.contains("判定 1bit+FS"), "{text}");
    }

    #[test]
    fn the_report_renders_the_profile_then_one_line_per_volume_and_per_page() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        // 判据值从公开 seam 上真算一个：整页偏 8 级，判据读出的就是 8.000。
        let four_bit = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                outcome: PageOutcome::Whole(Processed {
                    // 正好两倍面板的一页：报告要说出它预缩过。
                    scaling: Scaling::plan(Size::new(2528, 3360), Size::new(1264, 1680)),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore {
                            candidate,
                            score: four_bit,
                        }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::LowestWithinThreshold,
                        },
                    },
                }),
            },
        );

        let text = super::report(&report, Mode::Process);

        // profile 一行、判据形状一行、卷六行（去处、几何门、卷级、驱动页、读取、缓存），
        // 页两行：一行几何，一行判定。
        assert_eq!(text.lines().count(), 10);
        // 头一行说明这份输出是给哪台设备的，以及本次用的面板。
        assert!(text.contains("kobo-libra-2"), "{text}");
        assert!(text.contains("300 PPI"), "{text}");
        assert!(text.contains("16 级灰阶"), "{text}");
        assert!(text.contains("library/volume-a"), "{text}");
        assert!(text.contains("1 页"), "{text}");
        assert!(text.contains("1264×1680"), "{text}");
        // 每页的缩放三件套：总缩放比、有没有预缩、残差比。
        assert!(text.contains("缩放比 2.000"), "{text}");
        assert!(text.contains("预缩 2×"), "{text}");
        assert!(text.contains("残差比 1.000"), "{text}");
        assert!(text.contains("out/volume-a/001.png"), "{text}");
        // 判定、它的理由，以及判定所依据的那个量：判定要可解释（spec 的 story 7）。
        assert!(
            text.contains(&format!(
                "判定 4bit（阈值内最低的一档）  判据 4bit {four_bit}"
            )),
            "{text}"
        );
        // 阈值对整份报告只有一个，写在头一行的 profile 里，并标明它是怎么定出来的。
        assert!(
            text.contains("阈值 5.500（盲测标定于 boox-poke6，其余面板未复核）"),
            "{text}"
        );
        // 判据那一栏的每个数都是分块聚合收出来的，而聚合里的 K 同样没标定——
        // 不说出来，读的人无从判断这一栏该信到什么程度（02 号票，ADR 0002 决定第 3 条）。
        // 块边长是 ADR 定死的数，直接写；K 是占位值，从 `aggregation()` 取——
        // 标定把它换掉时这一条不该跟着改。
        assert!(text.contains("判据聚合 分块 32×32"), "{text}");
        assert!(
            text.contains(&format!("不宽于 {} 块", aggregation().tail_tiles)),
            "{text}"
        );
        assert!(text.contains("K 未标定占位值"), "{text}");
        // 卷成为不可分割的处理单元是 ADR 0005 认下的代价：用量与有没有溢写都要说出来。
        assert!(text.contains("缓存 1 页 1.0 MiB"), "{text}");
        assert!(text.contains("未溢写"), "{text}");
        // 「这卷为什么是这个候选」要有一个指得出驱动页的答案（ADR 0006）。
        assert!(text.contains("卷级 基准档 4bit"), "{text}");
        assert!(text.contains("驱动页 library/volume-a/001.jpg"), "{text}");
        // 上包络不承诺卷内绝对一致：离群与迟滞升档各出了多少页，报告要说出来。
        // 离群那一处还带着占比——「一页都没摘出来」要在报告里看得见，而光看计数分不清
        // 「本来就没有离群页」与「离群判定整个失灵」（加固批 01 号票）。
        assert!(text.contains("离群 0 页（0.0%）"), "{text}");
        assert!(text.contains("迟滞升档 0 页"), "{text}");
        // 上包络的分位、迟滞页数、离群页判据的立脚点分位与倍数，四者均未标定，
        // 报告显式标注（ADR 0006）。
        assert!(text.contains("四者均未标定"), "{text}");
        // 几何门的判定范围与本卷的抖动模式都要报出来（ADR 0007、06 号票）：
        // 这一页在范围内、门开着，那个「不抖动」因此是判据选的。
        assert!(
            text.contains("几何门 判定范围 灰度页 1 页 · 不成立 0 页 · 本卷 不抖动"),
            "{text}"
        );
    }

    /// 几何门那一段要说出**判定范围**与**被排除的页**（06 号票）：门逐页判，
    /// 「不成立」这句话得连着「范围里有几页、是哪几页」一起读，用户才判断得了这一卷该怎么办。
    #[test]
    fn a_broken_geometry_gate_names_its_scope_and_the_pages_it_left_out() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Two, Dither::Off);
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![170]));
        let score = tonefit::score(&reference, &tonefit::quantize(reference.image(), candidate));
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(800, 1000),
                outcome: PageOutcome::Whole(Processed {
                    // 源比目标小：按不放大原样输出，一条边都贴不住面板。
                    scaling: Scaling::plan(Size::new(800, 1000), Size::new(800, 1000)),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Broken,
                        scores: vec![CandidateScore { candidate, score }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::VolumeEnvelope,
                        },
                    },
                }),
            },
        );

        let text = super::report(&report, Mode::Process);

        // 判定范围与不成立的页数并排：一卷全是彩页时门同样成立，那是「无人可关」，
        // 不是「每一页都贴住了面板」。
        assert!(
            text.contains("几何门 判定范围 灰度页 1 页 · 不成立 1 页 · 本卷 不抖动"),
            "{text}"
        );
        // 这一卷范围内一页都不成立：没有别人可护，卷级那一档由它们自己定出。
        assert!(text.contains("范围内一页都不成立"), "{text}");
        // 同一道门也撑着面板灰阶那道硬上界（ADR 0003），它跟着失效这件事不能只留在注释里。
        assert!(
            text.contains("面板灰阶上界的依据在这几页上随门一起失效"),
            "{text}"
        );
        assert!(text.contains("ADR 0003"), "{text}");
    }

    /// 报告要区分彩页与灰度页，也要区分它们走了哪条分支（10 号票）。
    ///
    /// 三页各占一种情形：彩页走彩色分支、彩页转灰走灰度路径、灰度页走灰度路径。
    /// 中间那一种是最容易被藏起来的——它有判定，看上去与灰度页毫无二致，
    /// 而用户想知道的恰恰是「这台设备为什么没留住颜色」。
    #[test]
    fn the_report_tells_a_color_page_apart_from_a_gray_one() {
        let profile = Profile::resolve("kobo-libra-colour").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let page = |name: &str, color, branch| PageReport {
            source: PathBuf::from(format!("library/volume-a/{name}.png")),
            output: PathBuf::from(format!("out/volume-a/{name}.png")),
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Whole(Processed {
                scaling: typical_scaling(),
                color,
                branch,
            }),
        };
        let gray_branch = || PageBranch::Gray {
            gate: GeometryGate::Holds,
            scores: vec![CandidateScore { candidate, score }],
            verdict: Verdict {
                candidate,
                reason: Reason::LowestWithinThreshold,
            },
        };
        let report = Report {
            profile,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                // 驱动页必须是一张灰度页：彩页不进上包络，指不出档来。
                verdict: Some(VolumeVerdict::Envelope(Envelope {
                    base: candidate,
                    driver: 2,
                    body_pages: 2,
                    outlier_pages: 0,
                    raised_pages: 0,
                })),
                cache: cache_usage(),
                io: io_plan(),
                decodes: 3,
                pages: vec![
                    page("001", PageColor::Color, PageBranch::Color),
                    page("002", PageColor::Color, gray_branch()),
                    page("003", PageColor::Gray, gray_branch()),
                ],
            }],
        };

        let text = super::report(&report, Mode::Process);

        // 卷那一行数得出彩页有几张：走哪条分支不影响它是不是彩页。
        assert!(text.contains("3 页，其中彩页 2 页"), "{text}");
        // 彩色分支那一页说得出它为什么没有判定。
        assert!(text.contains("彩页 · 彩色分支：只缩放"), "{text}");
        assert!(text.contains("不进灰度缓存也不进卷级上包络"), "{text}");
        // 转灰的那一页有判定，行首标着它的来路。
        assert!(text.contains("彩页转灰 · 判定 4bit"), "{text}");
        // 灰度页那一行不多带任何标记：四个空格之后直接是判定。
        assert!(text.contains("    判定 4bit"), "{text}");
        assert!(text.contains("驱动页 library/volume-a/003.png"), "{text}");
    }

    /// 跳过的卷只占两行：去处那一行，加上说清它为什么什么都没有的那一行。
    ///
    /// 几何门、卷级判定、缓存用量、逐页结果一个都不出现——那一趟根本没算过它们，
    /// 报告摆出任何一项都是编的。页数照旧要说出来：它是源那一侧的事实。
    #[test]
    fn a_skipped_volume_says_so_and_says_nothing_it_did_not_compute() {
        let report = Report {
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                pages: Vec::new(),
                verdict: Some(VolumeVerdict::Skipped { page_count: 12 }),
                cache: cache_usage(),
                io: io_plan(),
                decodes: 0,
            }],
        };

        let text = super::report(&report, Mode::Process);

        // profile 一行、判据形状一行、卷两行，加上读取那一行——跳过的卷同样把整卷读了一遍。
        assert_eq!(text.lines().count(), 5);
        assert!(
            text.contains("library/volume-a → out/volume-a（12 页）"),
            "{text}"
        );
        assert!(text.contains("跳过 幂等命中"), "{text}");
        // 改哪一项会让它重做，用户得看得见（spec 的 story 9）。
        assert!(text.contains("工具版本、profile、参数、源均未变"), "{text}");
        assert!(!text.contains("几何门"), "{text}");
        assert!(!text.contains("缓存"), "{text}");
    }

    /// 介质探不出来的卷：报告说得出它退到了串行，也说得出**为什么**探不出来（13 号票）。
    ///
    /// 不说那句话，退到保守策略这件事对用户就只表现为「这一卷跑得慢」——
    /// 而那正是他没法据以决定要不要 `--io-mode concurrent` 的样子。
    #[test]
    fn a_volume_whose_medium_is_unknown_says_why_it_fell_back_to_serial() {
        let mut report = Report {
            profile: Profile::resolve("kobo-libra-2").expect("内置型号"),
            volumes: vec![VolumeReport {
                volume: PathBuf::from(r"\\nas\share\volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                pages: Vec::new(),
                verdict: Some(VolumeVerdict::Skipped { page_count: 12 }),
                cache: cache_usage(),
                io: io_plan(),
                decodes: 0,
            }],
        };
        report.volumes[0].io = IoPlan {
            medium: Medium::Unknown {
                reason: r"\\nas\share\ 是网络路径，介质无从探测".to_owned(),
            },
            readers: 1,
            chosen_by: ChosenBy::Probe,
        };

        let text = super::report(&report, Mode::Process);

        assert!(text.contains("读取串行"), "{text}");
        assert!(text.contains("是网络路径"), "{text}");
    }

    /// 被隔离的卷要说清三件事：几页失败、整卷去了哪儿、每一页各是为什么
    /// （spec 的 story 25、story 26）。退出码跟着分开——脚本读的是那个数。
    #[test]
    fn an_isolated_volume_names_its_failed_pages_and_gets_its_own_exit_code() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let good = PageReport {
            source: PathBuf::from("library/volume-a/001.jpg"),
            output: PathBuf::from("out/_isolated/volume-a/001.png"),
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Whole(Processed {
                scaling: typical_scaling(),
                color: PageColor::Gray,
                branch: PageBranch::Gray {
                    gate: GeometryGate::Holds,
                    scores: vec![CandidateScore { candidate, score }],
                    verdict: Verdict {
                        candidate,
                        reason: Reason::VolumeEnvelope,
                    },
                },
            }),
        };
        let failed = PageReport {
            source: PathBuf::from("library/volume-a/002.jpg"),
            output: PathBuf::from("out/_isolated/volume-a/002.png"),
            // 失败页照卷内统一尺寸出：与上面那张好页一模一样。
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Failed {
                reason: "解 library/volume-a/002.jpg 这一页: 判定格式".to_owned(),
            },
        };
        let report = Report {
            profile,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/_isolated/volume-a"),
                // 上一趟这一卷是干净的，那一份还在 out/volume-a 留着。
                superseded: Some(PathBuf::from("out/volume-a")),
                // 驱动页必须是一张好页：失败页没有判据曲线，指不出档来。
                verdict: Some(VolumeVerdict::Envelope(envelope(candidate))),
                cache: cache_usage(),
                io: io_plan(),
                decodes: 2,
                pages: vec![good, failed],
            }],
        };

        let text = super::report(&report, Mode::Process);

        // 卷级那一行说得出几页失败、整卷去了哪儿。
        assert!(text.contains("隔离 1 页失败"), "{text}");
        assert!(text.contains("out/_isolated/volume-a"), "{text}");
        // 隔离的卷仍是**处理过**的卷：几何门、卷级判定、缓存一样不少。
        // 失败页不在几何门的判定范围内（它连尺寸都没有），范围因此只有那一张好页。
        assert!(
            text.contains("几何门 判定范围 灰度页 1 页 · 不成立 0 页"),
            "{text}"
        );
        assert!(text.contains("卷级 基准档 4bit"), "{text}");
        // 失败页那两行：尺寸从哪来，以及它为什么失败。
        assert!(
            text.contains("1264×1680  失败页 · 卷内统一尺寸留白"),
            "{text}"
        );
        assert!(
            text.contains("失败 解 library/volume-a/002.jpg 这一页: 判定格式"),
            "{text}"
        );
        // 末尾那一行：几十卷跑下来不用往回翻也知道这一趟出过事。
        assert!(text.contains("隔离 1 卷 · 失败 1 页"), "{text}");
        // 上一趟写在干净去处的那一份还在，这一趟没覆盖它——报告要指名道姓说出来。
        assert!(text.contains("过期副本 out/volume-a"), "{text}");
        assert!(text.contains("删不删由你"), "{text}");
        // 退出码分得开「全部成功」与「有卷被隔离」。
        assert_eq!(exit_code(&report), ISOLATED_EXIT);
    }

    /// 部分救回页在报告里认得出来，而且**只有报告认得出来**（04 号票）。
    ///
    /// 它没有退出码替它喊，卷也照旧落在干净的去处：这一趟从进程那一侧看与全部成功
    /// 一模一样。三处各说一遍——逐页那一行说这一页救回了多少，卷级那一行说它没参与
    /// 卷级的判定，末尾那一行让几十卷跑下来的人不用往回翻。
    #[test]
    fn the_report_marks_a_salvaged_page_and_says_it_stayed_out_of_the_volume_decision() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let processed = |reason| Processed {
            scaling: typical_scaling(),
            color: PageColor::Gray,
            branch: PageBranch::Gray {
                gate: GeometryGate::Holds,
                scores: vec![CandidateScore { candidate, score }],
                verdict: Verdict { candidate, reason },
            },
        };
        let whole = PageReport {
            source: PathBuf::from("library/volume-a/001.jpg"),
            output: PathBuf::from("out/volume-a/001.png"),
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Whole(processed(Reason::VolumeEnvelope)),
        };
        let salvaged = PageReport {
            source: PathBuf::from("library/volume-a/002.jpg"),
            output: PathBuf::from("out/volume-a/002.png"),
            // 它按**自己**的尺寸出：文件头里那个尺寸一点没缺。
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Salvaged {
                // 它没进上包络，判定因此是它自己那条判据曲线定的。
                page: processed(Reason::LowestWithinThreshold),
                salvage: Salvage::from_share(0.625),
            },
        };
        let report = Report {
            profile,
            volumes: vec![VolumeReport {
                volume: PathBuf::from("library/volume-a"),
                output: PathBuf::from("out/volume-a"),
                superseded: None,
                verdict: Some(VolumeVerdict::Envelope(envelope(candidate))),
                cache: cache_usage(),
                io: io_plan(),
                decodes: 2,
                pages: vec![whole, salvaged],
            }],
        };

        let text = super::report(&report, Mode::Process);

        // 逐页那一行：救回了多少，摆在判定前面。
        assert!(text.contains("救回 62.5% · 判定 4bit"), "{text}");
        // 卷级那一行：这一卷有几页不全，以及它没参与卷级的哪一件事。
        // 几何门不在那句话里了——门逐页判之后照旧问它（ADR 0007 决定第 1 条，06 号票）。
        assert!(text.contains("部分救回 1 页"), "{text}");
        assert!(text.contains("不参与卷级上包络"), "{text}");
        assert!(text.contains("几何门照旧问它们"), "{text}");
        // 末尾那一行：几十卷跑下来不用往回翻。
        assert!(text.contains("部分救回 1 卷 · 1 页"), "{text}");
        // 完好的那一页一个字都不多说：它那一行以判定开头，前面没有救回那一截。
        assert!(
            text.contains(
                "
    判定 4bit（卷级上包络）"
            ),
            "{text}"
        );
        // 卷没被隔离，退出码因此仍是 0——报告是唯一说得出这件事的地方。
        assert!(!text.contains("隔离"), "{text}");
        assert_eq!(exit_code(&report), SUCCESS_EXIT);
    }

    /// 一卷都没被隔离时，隔离那几行一个字都不出现，退出码是 0。
    ///
    /// 「没出事」与「出了事」在报告与退出码上都得分得开，而分得开要两侧各测一遍。
    #[test]
    fn a_run_without_a_failed_page_says_nothing_about_isolation() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let reference = Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128]));
        let score = tonefit::score(&reference, &tonefit::quantize(reference.image(), candidate));
        let report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            PageReport {
                source: PathBuf::from("library/volume-a/001.jpg"),
                output: PathBuf::from("out/volume-a/001.png"),
                size: Size::new(1264, 1680),
                outcome: PageOutcome::Whole(Processed {
                    scaling: typical_scaling(),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore { candidate, score }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::VolumeEnvelope,
                        },
                    },
                }),
            },
        );

        let text = super::report(&report, Mode::Process);

        assert!(!text.contains("隔离"), "{text}");
        assert!(!text.contains("失败"), "{text}");
        assert!(!text.contains("过期副本"), "{text}");
        // 一页都没救回过的一趟同样一个字都不说（04 号票）。
        assert!(!text.contains("救回"), "{text}");
        assert_eq!(exit_code(&report), SUCCESS_EXIT);
    }

    /// 四段各画各的，拼起来与一次性渲染出的**逐字节相同**（会话批的 02、09 号票）。
    ///
    /// 会话就是这么画的：抬头一次，卷级与逐页逐卷出，末尾收口。这一条钉住的是
    /// 「两边措辞只有一套」——真有人在 [`report`] 里插了一行别处没有的东西，这里当场红。
    #[test]
    fn drawing_the_four_parts_one_by_one_gives_the_same_bytes_as_one_shot() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let candidate = Candidate::new(BitDepth::Four, Dither::Off);
        let score = tonefit::score(
            &Reference::new(profile.panel(), GrayImage::new(Size::new(1, 1), vec![128])),
            &GrayImage::new(Size::new(1, 1), vec![136]),
        );
        let salvaged = PageReport {
            source: PathBuf::from("library/volume-a/001.jpg"),
            output: PathBuf::from("out/volume-a/001.png"),
            size: Size::new(1264, 1680),
            outcome: PageOutcome::Salvaged {
                page: Processed {
                    scaling: typical_scaling(),
                    color: PageColor::Gray,
                    branch: PageBranch::Gray {
                        gate: GeometryGate::Holds,
                        scores: vec![CandidateScore { candidate, score }],
                        verdict: Verdict {
                            candidate,
                            reason: Reason::LowestWithinThreshold,
                        },
                    },
                },
                salvage: Salvage::from_share(0.625),
            },
        };
        // 两卷：一卷带着部分救回页（末尾那一小结因此在场），一卷是跳过的（它没有逐页那一段）。
        let mut report = one_page_report(
            profile,
            VolumeVerdict::Envelope(envelope(candidate)),
            salvaged,
        );
        report.volumes.push(VolumeReport {
            volume: PathBuf::from("library/volume-b"),
            output: PathBuf::from("out/volume-b"),
            superseded: None,
            pages: Vec::new(),
            verdict: Some(VolumeVerdict::Skipped { page_count: 12 }),
            cache: cache_usage(),
            io: io_plan(),
            decodes: 0,
        });

        let mut drawn = header(&report, Mode::Process);
        for each in &report.volumes {
            drawn.push_str(&volume(each));
            drawn.push_str(&pages(each));
        }
        drawn.push_str(&tail(&report));

        assert_eq!(drawn, super::report(&report, Mode::Process));
    }
}
