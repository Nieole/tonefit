//! 样张：一张图走满管线，这块面板上这一页派得出的**每一个**候选各编一张，
//! 连同《参照》一张落进点名的目录（`CONTEXT.md` 的《样张》）。
//! 彩色面板上的彩页例外：它走彩色分支、不量化，那一叠只有 `run` 会写出的那一张。
//!
//! 库的第四个 seam（[`crate::write_proof`]）落在这里，**它不另写一条管线**：
//! 打开一张源页（[`open_source_page`]）、量一张灰度页（[`examine_gray_page`]）、
//! 按一个候选编一张（[`candidate_bytes`]）、编彩色分支上那一张（[`color_bytes`]），
//! 走的都是转换那一趟的同一批函数（都住在 `crate::pipeline`），只是不经过卷、缓存与汇总那一层
//! （spec《Implementation Decisions》第二条）。
//! 另写一条平行的路，两条路迟早各自漂移——而样张要回答的正是「写出去会是什么样」。
//! 钉住这一句的是 `tests/proof.rs` 里拿 `run` 当神谕的那几条。
//!
//! 印在终端上的那几行不在这里：那是**界面文案**，随调用方走（见二进制侧的 `render`），
//! 与灰阶测试图同一条规矩。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::color::ColorImage;
use crate::decide::{self, CandidateScore, Verdict};
use crate::geometry::Fit;
use crate::metadata::MemberPart;
use crate::pipeline::{
    Candidates, Examined, Opened, Piece, Pieces, Settles, WhiteWhenOff, candidate_bytes,
    color_bytes, examine_gray_page, open_source_page, output_name,
};
use crate::quantize::{BitDepth, Candidate};
use crate::report::{PageBranch, PageOutcome, PageReport, Processed};
use crate::{
    GeometryGate, GrayImage, PageColor, Request, Salvage, Scaling, WhiteAlignment, decode, encode,
    is_archive, resample,
};

/// 一张图出的样张：**一张输出页一叠**，按阅读顺序（spec《Implementation Decisions》第六条）。
///
/// 按输出页分、不按源页分：拆开的跨页真会被写出去的是那两半，而不是一张没人会看到的整页。
#[derive(Debug, Clone)]
pub struct Proof {
    pub pages: Vec<ProofPage>,
}

/// 一张输出页的那一叠：灰度路径上这一页的每一个候选各一张，连同《参照》一张；
/// 彩色分支上只有一张。
#[derive(Debug, Clone)]
pub struct ProofPage {
    /// 这一页的几何、分支，灰度路径上还有判定、画质分曲线、尺寸贴合检查与纸色提白。
    ///
    /// **与报告里的一页同一个形状**，不另起一个：几件事都是转换那一趟本来就说得出的，
    /// 界面层说它们的那一套措辞因此照搬得动（spec 第九条：措辞从既有出处取）。
    /// 走的是哪条分支由它说（[`PageReport::branch`]）。
    /// `output` 指着**转换那一趟写出去的那一张**——灰度路径上是判定那一档的那一张，
    /// 彩色分支上就是唯一那一张（神谕那几条比的都是它）。
    pub page: PageReport,
    /// 这一叠落到盘上的那几张，**跟着 `page` 的分支走**：[`Sheets`] 的两种与 [`PageBranch`]
    /// 的两种一一对应。类型拦不住两者对不上，拦住它的是构造——两样在落盘那一步的**同一个分支**里
    /// 一起造出来，别处一个都不造。
    pub sheets: Sheets,
}

impl ProofPage {
    /// 这一页的每一个候选，连同它的画质分与它那一张，由小到大。彩色分支上一个都没有。
    ///
    /// 「第几张是哪一档」只在这里配一次：曲线与那几张逐格同序是 [`crate::write_proof`] 的承诺，
    /// 两边的调用方各自去配，早晚有一处配错位。
    pub fn scored(&self) -> impl Iterator<Item = (&CandidateScore, &Sheet)> {
        let scores = self.page.scores();
        let candidates: &[Sheet] = match &self.sheets {
            Sheets::Gray { candidates, .. } => candidates,
            Sheets::Color(_) => &[],
        };
        assert_eq!(
            scores.len(),
            candidates.len(),
            "曲线上一个候选一张：{} 的样张对不上它的画质分曲线",
            self.page.source.display()
        );
        scores.iter().zip(candidates)
    }
}

/// 一叠样张落到盘上的那几张，**跟着这一页走的那条分支**（[`PageBranch`]）。
///
/// 两种而不是几个各自可空的字段，与 [`PageBranch`] 同一条理由：两条分支出的不是同一套——
/// 彩色分支不量化，没有候选可比，也就没有「量化之前长什么样」的参照可对照
/// （样张 spec《Implementation Decisions》第七条）。
#[derive(Debug, Clone)]
pub enum Sheets {
    /// 灰度路径：每一个候选各一张，连同《参照》一张。
    Gray {
        /// 《参照》那一张：8 位无损写出，没被量化过（spec 第四条：对照本身不许带自己的损伤）。
        reference: Sheet,
        /// 每一个候选各一张，与这一页的画质分曲线**逐格同序**（由小到大）。
        ///
        /// 它不自己再记一遍是哪一档、画质分多少：那两样在曲线上只有一处出处，
        /// 要成对地读走 [`ProofPage::scored`]。
        candidates: Vec<Sheet>,
    },
    /// 彩色分支：只有一张，就是 `run` 会写出的那一张（ADR 0005 决定第 4 条）。
    Color(Sheet),
}

impl Sheets {
    /// 这一叠落到盘上的每一张：灰度路径上候选那几张由小到大、参照收尾；彩色分支上就是那一张。
    pub fn iter(&self) -> impl Iterator<Item = &Sheet> {
        let (candidates, last): (&[Sheet], &Sheet) = match self {
            Sheets::Gray {
                reference,
                candidates,
            } => (candidates, reference),
            Sheets::Color(sheet) => (&[], sheet),
        };
        candidates.iter().chain([last])
    }
}

/// 样张里的一张：落在哪儿、多大。
///
/// 它是哪一张不记在这里：由它在 [`Sheets`] 里的位置说——参照、彩色分支那一张各有自己那一格，
/// 候选那几张各是哪一档由 [`ProofPage::scored`] 配上。
#[derive(Debug, Clone)]
pub struct Sheet {
    /// 落在哪个文件上：点名的去处接上这一张的名字——这一页在 `run` 那一侧的成员名，
    /// 接上它是哪一张：候选那几张接它是哪一档（`001.2bit+FS.png`），参照那一张与彩色分支那一张
    /// 接的是词条名（`001.参照.png`、`001.彩色分支.png`）。
    pub file: PathBuf,
    /// 写出去多少字节：体积与画质两轴要在同一屏上比得了（spec 的 story 16）。
    pub bytes: u64,
}

/// 《参照》那一张文件名里的那一截：取词条名。
const REFERENCE: &str = "参照";

/// 彩色分支上那一张文件名里的那一截：取词条名（《灰度路径 / 彩色分支》）。
const COLOR_BRANCH: &str = "彩色分支";

/// 出一张图的样张，见 [`crate::write_proof`]。
///
/// 答不出来的那几种各在哪一步说、各说什么，见那一处的《说不出话的那几种》；
/// 这里只守次序：**先全部编好，再落盘**——落盘之前的每一种拒绝都发生在第一个字节写出去之前，
/// 去处本来不在的话连目录都不建，不留一叠半成品让人对着猜。
pub(crate) fn write(source: &Path, request: &Request, out: &Path) -> Result<Proof> {
    // 判定从哪几个里挑，照转换那一趟裁（见 [`draft_gray`]）。覆盖项越界（点名一档面板写不出）
    // 的那句拒绝也在这里说，而它排在**头一行**、读图之前：转换那一趟碰卷之前就说它
    // （`ensure_the_overrides_leave_a_candidate`），同一份两处都错的请求上两条路因此先说同一句
    // （停车场 Q1042）；彩色分支上的页也逃不过——那一趟里它是这一卷的一页。
    let judged = Candidates::new(request)?;
    ensure_one_image(source)?;
    let bytes = std::fs::read(source).with_context(|| format!("读 {}", source.display()))?;
    // 解码器与缩放器现开一个、一眼都不看：窄计数器要的是「记在动作本身上」，
    // 而账本是谁的由调用方说了算（与 `examine_gray_page` 那一段同一条）。
    //
    // 解码解不开的那几种就是转换那一趟的**坏页**（`CONTEXT.md` 的《失败》）：那一趟占一格白页，
    // 样张一张都不出（spec 的 story 24）。
    let Opened {
        color,
        salvage,
        pieces,
    } = open_source_page(&bytes, request, &decode::Decoder::default())
        .with_context(|| format!("{} 解不开，是一张坏页：样张一张都没出", source.display()))?;
    // 这一页在 `run` 那一侧的成员名从它推出（见 `crate::pipeline::output_name`）。
    let name = source
        .file_name()
        .map(Path::new)
        .with_context(|| format!("{} 不是一张图", source.display()))?;
    let resampler = resample::Resampler::default();
    let parts = MemberPart::family(name, pieces.len());
    // 走哪条分支由**面板与页**共同决定，那一问在 `open_source_page` 里、只问一次：
    // 样张照它交出来的那一支走，不自己另判（ADR 0005 决定第 4 条）。
    //
    // **切出来的每一块各出一叠**，按阅读顺序：样张按输出页出，不按源页（spec 第六条；
    // 停车场 Q996 判的是这一条）。名字照 `run` 给那一块的输出页名（`output_name`）。
    let drafted = match pieces {
        Pieces::Gray(pieces) => draft_gray(source, request, &judged, &resampler, pieces)?,
        Pieces::Color(pieces) => draft_color(source, request, &resampler, pieces)?,
    };
    std::fs::create_dir_all(out)
        .with_context(|| format!("{CANNOT_WRITE}：去处 {} 建不出来", out.display()))?;
    let pages = drafted
        .into_iter()
        .zip(parts)
        .map(|(drafted, part)| drafted.land(source, out, &output_name(part), color, salvage))
        .collect::<Result<Vec<_>>>()?;
    Ok(Proof { pages })
}

/// 样张只认**一张图**（`proof-sheet/06`，spec 的 story 25）：点成别的东西，
/// 当场一句话说清，一个字节都不读。「一张图」照转换那一趟认：**卷里的一页**。
///
/// - **一个目录、一个归档**：那两样在转换那一趟是**卷**（`CONTEXT.md` 的《卷》），
///   归档认哪几个扩展名与那一趟同一把尺子（[`is_archive`]）。句子后半截说该怎么改——
///   要看一卷，spec 的《Out of Scope》给过路：先跑一趟预览看读数，挑出可疑的那几页再逐页出样张。
/// - **透传文件**：扩展名不是页的成员，转换那一趟原样拷它（`CONTEXT.md` 的《成员》）；
///   认不认它是一页只看扩展名，与那一趟同一把尺子（[`decode::is_page`]）。
///   字节恰好解得开也不出——那一趟一张都不会编出去，样张没有「写出去会是什么样」可答
///   （停车场 Q1055）。
///
/// **只问盘上真在的东西**：三问都只看路径的形状，对一个不存在的路径会说错话——
/// 敲错的 `卷1` 会被说成透传文件，不在的 `合集.cbz` 会被说成归档。不在的那一种放过去，
/// 由读盘那一步说它读不到。
fn ensure_one_image(source: &Path) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    let (what, instead) = if source.is_dir() {
        ("一个目录", VOLUME_INSTEAD_OF_PAGE)
    } else if is_archive(source) {
        ("一个归档", VOLUME_INSTEAD_OF_PAGE)
    } else if !decode::is_page(source) {
        (
            "透传文件",
            "转换那一趟原样拷它、一张都不编，样张没有东西可比",
        )
    } else {
        return Ok(());
    };
    bail!(
        "{TAKES_ONE_IMAGE}，{} 是{what}：{instead}。",
        source.display()
    )
}

/// 点成别的东西时那几句拒绝打头的那半句。
const TAKES_ONE_IMAGE: &str = "样张只认一张图";

/// 点成一卷（目录或归档）时那句拒绝的后半截：该怎么改。
const VOLUME_INSTEAD_OF_PAGE: &str = "点名里面要看的那一页（归档要先把它取出来）。\
     要看一整卷，先跑一趟 --dry-run 看读数，挑出可疑的那几页再逐页出样张";

/// 灰度路径上切出来的那几块，每一块编好一叠：每一个候选一张，外加《参照》一张。
///
/// 两套候选各管一件事（样张 spec《Implementation Decisions》第三条）：
/// 出哪几张照**两道界**裁（`shown`），判定从哪几个里挑照转换那一趟裁（`judged`）——
/// 覆盖项裁掉的是「这一趟不要」，不是「这一页不可能」。覆盖项越界的那句拒绝
/// 也由后者照转换那一趟说，一个字不另写。
///
/// 尺寸未贴合屏幕的那一块候选里没有抖动那一维（ADR 0007 决定第 2 条）：`shown` 按这一块的门给
/// （[`Candidates::for_gate`]），与转换那一趟在同一页上用的是同一套。
///
/// 每一块先量完，判定一起下（见 [`verdicts`]）：「覆盖项顶死没有」是开卷之前那一问，整张图问一次，
/// 不拿某一块剩下几个候选去问——那样问会答错（停车场 Q1014；one-source/02）。
fn draft_gray(
    source: &Path,
    request: &Request,
    judged: &Candidates,
    resampler: &resample::Resampler,
    pieces: Vec<(GrayImage, Piece)>,
) -> Result<Vec<Drafted>> {
    let shown = Candidates::without_overrides(&request.profile);
    let measured = pieces
        .into_iter()
        .map(|(image, piece)| {
            let examined = examine_gray_page(
                source,
                &image,
                request,
                &shown,
                resampler,
                WhiteWhenOff::Foresee,
            )?;
            let scores = judged_scores(source, &image, request, judged, &examined)?;
            Ok((examined, piece, scores))
        })
        .collect::<Result<Vec<_>>>()?;
    let verdicts = verdicts(
        request,
        judged,
        &measured
            .iter()
            .map(|(_, _, scores)| scores.as_slice())
            .collect::<Vec<_>>(),
    );
    measured
        .into_iter()
        .zip(verdicts)
        .map(|((examined, piece, _), verdict)| {
            let reference = encode::png(examined.reference.image(), BitDepth::Eight, None)?;
            let candidates = examined
                .scores
                .iter()
                .map(|score| {
                    // 记录交 `None`：样张不写《记录》（spec 第五条）。
                    // 判定那一档的那一张因此与 `run --no-metadata` 走的是同一个函数、同一组入参。
                    candidate_bytes(examined.reference.image(), score.candidate, None)
                        .map(|bytes| (score.candidate, bytes))
                })
                .collect::<Result<Vec<_>>>()?;
            let Examined {
                scores,
                gate,
                fit,
                scaling,
                white,
                ..
            } = examined;
            Ok(Drafted {
                piece,
                fit,
                scaling,
                sheets: DraftedSheets::Gray {
                    gate,
                    scores,
                    verdict,
                    white,
                    reference,
                    candidates,
                },
            })
        })
        .collect()
}

/// 彩色分支上切出来的那几块，每一块编好**一张**：`run` 会写出的那一张（ADR 0005 决定第 4 条）。
///
/// 这条路不量化：没有候选可比、没有画质分，也就没有参照可对照。它不进尺寸贴合检查
/// （ADR 0010 决定第 4 条），覆盖项在这里也无从说话——越界的那句拒绝在分流之前已经说过了。
/// 目标尺寸与转换那一趟同出 [`crate::FitMode::target`]，缩放与编码同出 [`color_bytes`]。
fn draft_color(
    source: &Path,
    request: &Request,
    resampler: &resample::Resampler,
    pieces: Vec<(ColorImage, Piece)>,
) -> Result<Vec<Drafted>> {
    let panel = request.profile.panel().resolution;
    pieces
        .into_iter()
        .map(|(image, piece)| {
            let fit = request.fit.target(image.size(), panel);
            // 记录交 `None`：样张不写《记录》（spec 第五条）——那一张因此与 `run --no-metadata`
            // 走的是同一个函数、同一组入参。
            let (scaling, bytes) =
                color_bytes(source, &image, fit.size(), request.filter, resampler, None)?;
            Ok(Drafted {
                piece,
                fit,
                scaling,
                sheets: DraftedSheets::Color(bytes),
            })
        })
        .collect()
}

/// 这一块上**判定从哪几格里挑**：画质分是整套求的，挑出转换那一趟在这一块上会留下的那几格。
///
/// 那几个是 `judged` 按这一块的门给的一套（见 [`Candidates::for_gate`]）：覆盖项没点时就是整套，
/// 点了就是它们留下的那几个。每一档的画质分各求各的，从整套里挑出那几格与转换那一趟当场只求那几格
/// **逐格相同**。
///
/// `--dither fs` 撞上一块没贴合屏幕（互锁 ③）时这里回的是转换那一趟的那句拒绝：
/// 那一趟一个字节都不写，样张就没有「判定那一张」可给。
fn judged_scores(
    source: &Path,
    image: &GrayImage,
    request: &Request,
    judged: &Candidates,
    examined: &Examined,
) -> Result<Vec<CandidateScore>> {
    let panel = request.profile.panel().resolution;
    let allowed = judged.for_gate(source, examined.gate, image.size(), panel)?;
    Ok(examined
        .scores
        .iter()
        .filter(|score| allowed.contains(&score.candidate))
        .copied()
        .collect())
}

/// 这张图每一块的《判定》，按阅读顺序：与转换那一趟把这张图摆成一卷时**逐格相同**，
/// 神谕那几条比的正是判定那一档写出去的那一张。
///
/// 进来的是每一块判定从中挑的那几格画质分（[`judged_scores`]）。默认那条路上灰阶档位
/// 逐页各判各的（ADR 0018 决定第 2 条），每一块拿自己那几格判；**「覆盖项顶死没有」是开卷之前那一问**，
/// 与转换那一趟分析环节、汇总问的是同一句（`Settles::if_processing`），顶死时理由是覆盖
/// （哪一种算顶死，见 `CONTEXT.md` 的《覆盖顶死》）。
///
/// 按块的候选集去问会答错：只点灰阶档位时，门不成立的那一块只剩一个候选，被说成顶死，
/// 而转换那一趟那一页的档是它自己那条曲线判出来的——字节相同，理由不同（停车场 Q1014）。
///
/// 整卷统一灰阶那条路不在这里：那一格样张不读（见 [`crate::write_proof`]）。那一问也不看它——
/// 顶死的那一档只有顶死那一趟有，没顶死时开着落在等整卷、不开落在这一页自己，两处都是 `None`
/// （见 `Settles::pinned`）。
fn verdicts(request: &Request, judged: &Candidates, pieces: &[&[CandidateScore]]) -> Vec<Verdict> {
    let pinned = Settles::if_processing(request, judged).pinned();
    pieces
        .iter()
        .map(|scores| decide::decide(scores, request.profile.threshold(), pinned))
        .collect()
}

/// 一叠编好、还没落盘的样张。
struct Drafted {
    piece: Piece,
    /// 目标尺寸，连同它是不是被兜底上界退回来的。
    fit: Fit,
    scaling: Scaling,
    sheets: DraftedSheets,
}

/// 一叠编好的那几张，跟着这一块走的那条分支——落盘之后就是 [`Sheets`] 那两种。
enum DraftedSheets {
    /// 灰度路径：这一块的门、画质分曲线、判定与纸色提白，连同编好的那几张。
    Gray {
        gate: GeometryGate,
        scores: Vec<CandidateScore>,
        verdict: Verdict,
        white: WhiteAlignment,
        /// 《参照》那一张编好的字节。
        reference: Vec<u8>,
        /// 每一个候选编好的字节，由小到大。
        candidates: Vec<(Candidate, Vec<u8>)>,
    },
    /// 彩色分支：唯一那一张编好的字节。
    Color(Vec<u8>),
}

impl Drafted {
    /// 以 `name`（这一块在 `run` 那一侧的成员名：`001.png`、`001-2.png`）写进去处，
    /// 拼出这一叠交给调用方的那一份。
    fn land(
        self,
        source: &Path,
        out: &Path,
        name: &Path,
        color: PageColor,
        salvage: Option<Salvage>,
    ) -> Result<ProofPage> {
        let Self {
            piece,
            fit,
            scaling,
            sheets,
        } = self;
        let (output, branch, sheets) = match sheets {
            DraftedSheets::Gray {
                gate,
                scores,
                verdict,
                white,
                reference,
                candidates,
            } => {
                let reference = written(out.join(sheet_name(name, REFERENCE)), &reference)?;
                let landed = candidates
                    .iter()
                    .map(|(candidate, bytes)| {
                        written(out.join(sheet_name(name, &candidate.to_string())), bytes)
                    })
                    .collect::<Result<Vec<_>>>()?;
                let output = candidates
                    .iter()
                    .zip(&landed)
                    .find(|((candidate, _), _)| *candidate == verdict.candidate)
                    .map(|(_, sheet)| sheet.file.clone())
                    .expect("判定出自这一页的曲线，那一档必在其中");
                (
                    output,
                    PageBranch::Gray {
                        gate,
                        scores,
                        verdict,
                        white,
                    },
                    Sheets::Gray {
                        reference,
                        candidates: landed,
                    },
                )
            }
            DraftedSheets::Color(bytes) => {
                let sheet = written(out.join(sheet_name(name, COLOR_BRANCH)), &bytes)?;
                (sheet.file.clone(), PageBranch::Color, Sheets::Color(sheet))
            }
        };
        let processed = Processed {
            crop: piece.crop,
            backstopped: fit.backstopped(),
            cut: piece.cut,
            spread_candidate: piece.candidate,
            scaling,
            color,
            branch,
        };
        Ok(ProofPage {
            page: PageReport {
                source: source.to_path_buf(),
                output,
                size: fit.size(),
                outcome: PageOutcome::of(processed, salvage),
            },
            sheets,
        })
    }
}

/// 样张里一张的文件名：**这一页在 `run` 那一侧的成员名**，接上它是哪一张。
///
/// 页那一截照 [`output_name`] 取（spec 第六条：与 `run` 给的输出页名同一套写法），
/// 拷进设备之后认得出是哪一页的第几张；候选那一截取 [`Candidate`] 的写法（`2bit+FS`），
/// 参照与彩色分支那一张取词条名。同一页的几张因此按名字排在一起。
fn sheet_name(page: &Path, what: &str) -> PathBuf {
    page.with_extension(format!("{what}.png"))
}

/// 写出一张，交回它落在哪儿、多大。写不进去（盘满、名字被占）回的是 [`CANNOT_WRITE`] 那一句，
/// 指着写不进的那一张。
///
/// 写到一半撞上的话，**已经落下的那几张留在去处里**，不回头收：与灰阶测试图那一路同一种朴素写法
/// （见 `calibrate::write_chart`），那句拒绝点得出卡在哪一张（停车场 Q1056）。
fn written(file: PathBuf, bytes: &[u8]) -> Result<Sheet> {
    std::fs::write(&file, bytes)
        .with_context(|| format!("{CANNOT_WRITE}：{} 写不进去", file.display()))?;
    Ok(Sheet {
        file,
        bytes: bytes.len() as u64,
    })
}

/// 落盘那一步出错时那两句打头的那半句：说得出是**写不出去**，不是图有毛病
/// （`proof-sheet/06`：解不开的图与写不进的去处，用户要改的是两样东西）。
const CANNOT_WRITE: &str = "样张写不出去";
