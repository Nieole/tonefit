//! 样张：一张图走满管线，这块面板上这一页派得出的**每一个**候选各编一张，
//! 连同《参照》一张落进点名的目录（`CONTEXT.md` 的《样张》）。
//!
//! 库的第四个 seam（[`crate::write_proof`]）落在这里，**它不另写一条管线**：
//! 打开一张源页（[`crate::open_source_page`]）、量一张灰度页（[`crate::examine_gray_page`]）、
//! 按一个候选编一张（[`crate::candidate_bytes`]），走的都是转换那一趟的同一批函数，
//! 只是不经过卷、缓存与汇总那一层（spec《Implementation Decisions》第二条）。
//! 另写一条平行的路，两条路迟早各自漂移——而样张要回答的正是「写出去会是什么样」。
//! 钉住这一句的是 `tests/proof.rs` 里拿 `run` 当神谕的那一条。
//!
//! 印在终端上的那几行不在这里：那是**界面文案**，随调用方走（见二进制侧的 `render`），
//! 与灰阶测试图同一条规矩。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::decide::{self, CandidateScore, Verdict};
use crate::quantize::{BitDepth, Candidate};
use crate::report::{PageBranch, PageOutcome, PageReport, Processed};
use crate::{
    Candidates, Examined, Opened, PageColor, Piece, Pieces, Request, Salvage, WhiteWhenOff,
    candidate_bytes, decode, encode, examine_gray_page, open_source_page, output_name, pinned,
    resample,
};

/// 一张图出的样张：**一张输出页一叠**，按阅读顺序（spec《Implementation Decisions》第六条）。
///
/// 按输出页分、不按源页分：拆开的跨页真会被写出去的是那两半，而不是一张没人会看到的整页。
#[derive(Debug, Clone)]
pub struct Proof {
    pub pages: Vec<ProofPage>,
}

/// 一张输出页的那一叠：这一页的每一个候选各一张，连同《参照》一张。
#[derive(Debug, Clone)]
pub struct ProofPage {
    /// 这一页的几何、判定、画质分曲线、尺寸贴合检查与纸色提白。
    ///
    /// **与报告里的一页同一个形状**，不另起一个：几件事都是转换那一趟本来就说得出的，
    /// 界面层说它们的那一套措辞因此照搬得动（spec 第九条：措辞从既有出处取）。
    /// `output` 指着**判定那一档的那一张**——转换那一趟写出去的就是它（神谕那一条比的也是它）。
    pub page: PageReport,
    /// 《参照》那一张：8 位无损写出，没被量化过（spec 第四条：对照本身不许带自己的损伤）。
    pub reference: Sheet,
    /// 这一页的每一个候选各一张，与 `page` 的画质分曲线**逐格同序**（由小到大）。
    ///
    /// 它不自己再记一遍是哪一档、画质分多少：那两样在曲线上只有一处出处，
    /// 要成对地读走 [`scored`](Self::scored)。
    pub candidates: Vec<Sheet>,
}

impl ProofPage {
    /// 这一页的每一个候选，连同它的画质分与它那一张，由小到大。
    ///
    /// 「第几张是哪一档」只在这里配一次：曲线与那几张逐格同序是 [`crate::write_proof`] 的承诺，
    /// 两边的调用方各自去配，早晚有一处配错位。
    pub fn scored(&self) -> impl Iterator<Item = (&CandidateScore, &Sheet)> {
        let scores = self.page.scores();
        assert_eq!(
            scores.len(),
            self.candidates.len(),
            "曲线上一个候选一张：{} 的样张对不上它的画质分曲线",
            self.page.source.display()
        );
        scores.iter().zip(&self.candidates)
    }
}

/// 样张里的一张：落在哪儿、多大。
///
/// 它是哪一张不记在这里：参照那一张是 [`ProofPage::reference`]，候选那几张各是哪一档
/// 由 [`ProofPage::scored`] 配上。
#[derive(Debug, Clone)]
pub struct Sheet {
    /// 落在哪个文件上：点名的去处接上这一张的名字——这一页在 `run` 那一侧的成员名，
    /// 接上它是哪一档（`001.2bit+FS.png`），参照那一张接的是词条名（`001.参照.png`）。
    pub file: PathBuf,
    /// 写出去多少字节：体积与画质两轴要在同一屏上比得了（spec 的 story 16）。
    pub bytes: u64,
}

/// 《参照》那一张文件名里的那一截：取词条名。
const REFERENCE: &str = "参照";

/// 出一张图的样张，见 [`crate::write_proof`]。
///
/// **先全部编好，再落盘**：解不开、撞上门的拒绝、编不出来，都发生在第一个字节写出去之前，
/// 半路出错时去处里一个文件都没有——不留一叠半成品让人对着猜。
pub(crate) fn write(source: &Path, request: &Request, out: &Path) -> Result<Proof> {
    let bytes = std::fs::read(source).with_context(|| format!("读 {}", source.display()))?;
    // 解码器与缩放器现开一个、一眼都不看：窄计数器要的是「记在动作本身上」，
    // 而账本是谁的由调用方说了算（与 `examine_gray_page` 那一段同一条）。
    let Opened {
        color,
        salvage,
        pieces,
    } = open_source_page(source, &bytes, request, &decode::Decoder::default())?;
    // 彩色分支那一叠（只出 `run` 会写出的那一张，说清走的是彩色分支）归 `proof-sheet/05`；
    // 在那之前这一支当场说清为什么出不了，而不是出一个空目录让人猜。
    let Pieces::Gray(pieces) = pieces else {
        bail!(
            "{} 在这块面板上走彩色分支：它不量化，没有候选可比",
            source.display()
        );
    };
    // 这一页在 `run` 那一侧的成员名从它推出（见 `crate::output_name`）。
    let name = source
        .file_name()
        .map(Path::new)
        .with_context(|| format!("{} 不是一张图", source.display()))?;
    let candidates = Candidates::new(request)?;
    let resampler = resample::Resampler::default();
    let count = pieces.len();
    let drafted = pieces
        .into_iter()
        .enumerate()
        .map(|(ordinal, (image, piece))| {
            let examined = examine_gray_page(
                source,
                &image,
                request,
                &candidates,
                &resampler,
                WhiteWhenOff::Foresee,
            )?;
            // 判定走的是转换那一趟在一页的卷上会走的那一句：逐页那条路上 `pinned` 答 `None`，
            // 覆盖项把候选裁到只剩一个时答那一档、理由是覆盖（见 `crate::pinned`）。
            let verdict = decide::decide(
                &examined.scores,
                request.profile.threshold(),
                pinned(request, &examined.scores),
            );
            let reference = encode::png(examined.reference.image(), BitDepth::Eight, None)?;
            let sheets = examined
                .scores
                .iter()
                .map(|score| {
                    // 记录交 `None`：样张不写《记录》（spec 第五条）。
                    // 判定那一档的那一张因此与 `run --no-metadata` 走的是同一个函数、同一组入参。
                    candidate_bytes(examined.reference.image(), score.candidate, None)
                        .map(|bytes| (score.candidate, bytes))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Drafted {
                name: output_name(name, ordinal, count),
                examined,
                piece,
                verdict,
                reference,
                sheets,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    std::fs::create_dir_all(out).with_context(|| format!("建样张的去处 {}", out.display()))?;
    let pages = drafted
        .into_iter()
        .map(|drafted| drafted.land(source, out, color, salvage))
        .collect::<Result<Vec<_>>>()?;
    Ok(Proof { pages })
}

/// 一叠编好、还没落盘的样张。
struct Drafted {
    /// 这一页在 `run` 那一侧的成员名（`001.png`、`001-2.png`）。
    name: PathBuf,
    examined: Examined,
    piece: Piece,
    verdict: Verdict,
    /// 《参照》那一张编好的字节。
    reference: Vec<u8>,
    /// 每一个候选编好的字节，由小到大。
    sheets: Vec<(Candidate, Vec<u8>)>,
}

impl Drafted {
    /// 写进去处，拼出这一叠交给调用方的那一份。
    fn land(
        self,
        source: &Path,
        out: &Path,
        color: PageColor,
        salvage: Option<Salvage>,
    ) -> Result<ProofPage> {
        let Self {
            name,
            examined,
            piece,
            verdict,
            reference,
            sheets,
        } = self;
        let reference = written(out.join(sheet_name(&name, REFERENCE)), &reference)?;
        let candidates = sheets
            .iter()
            .map(|(candidate, bytes)| {
                written(out.join(sheet_name(&name, &candidate.to_string())), bytes)
            })
            .collect::<Result<Vec<_>>>()?;
        let output = sheets
            .iter()
            .zip(&candidates)
            .find(|((candidate, _), _)| *candidate == verdict.candidate)
            .map(|(_, sheet)| sheet.file.clone())
            .expect("判定出自这一页的曲线，那一档必在其中");
        let Examined {
            scores,
            gate,
            fit,
            scaling,
            white,
            ..
        } = examined;
        let processed = Processed {
            crop: piece.crop,
            backstopped: fit.backstopped(),
            cut: piece.cut,
            spread_candidate: piece.candidate,
            scaling,
            color,
            branch: PageBranch::Gray {
                gate,
                scores,
                verdict,
                white,
            },
        };
        Ok(ProofPage {
            page: PageReport {
                source: source.to_path_buf(),
                output,
                size: fit.size(),
                outcome: PageOutcome::of(processed, salvage),
            },
            reference,
            candidates,
        })
    }
}

/// 样张里一张的文件名：**这一页在 `run` 那一侧的成员名**，接上它是哪一张。
///
/// 页那一截照 [`output_name`] 取（spec 第六条：与 `run` 给的输出页名同一套写法），
/// 拷进设备之后认得出是哪一页的第几张；候选那一截取 [`Candidate`] 的写法（`2bit+FS`），
/// 参照那一张取词条名。同一页的几张因此按名字排在一起。
fn sheet_name(page: &Path, what: &str) -> PathBuf {
    page.with_extension(format!("{what}.png"))
}

/// 写出一张，交回它落在哪儿、多大。
fn written(file: PathBuf, bytes: &[u8]) -> Result<Sheet> {
    std::fs::write(&file, bytes).with_context(|| format!("写样张 {}", file.display()))?;
    Ok(Sheet {
        file,
        bytes: bytes.len() as u64,
    })
}
