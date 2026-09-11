//! 输出容器：收页与透传文件，写成目录或 CBZ。
//!
//! 与源对称：输入是目录就写目录，输入是归档就写归档（[`crate::source::Container`] 定这件事）。
//! 归档的扩展名不随输入走，一律 `.cbz`（见 [`crate::source::OUTPUT_ARCHIVE_EXTENSION`]）。
//!
//! 幂等要读回上一趟的输出，读的也是这个容器，因此 [`Written`] 落在这里：
//! 「一页在容器里怎么找」两个方向上是同一件事，分开写就是两份会走散的容器知识。
//!
//! 两种形态**都先写到临时容器、收尾时才改名到最终位置**（见 [`Sink`]）。
//! 目录卷的去处里**借住着别的卷**时，收尾换掉的范围收窄到这一卷自己那几个成员
//! （见 [`Lodgers`]）。
//!
//! **按页跳过没有改这一层的形态**（two-pass-rework/14）：留下的页的字节从上一趟的输出里
//! 读回来（[`Written::bytes_of`]），**原样**照写页那条路写进这一趟的临时容器
//! （two-pass-rework/15：记录里没有要改写的卷级那一项了，搬就是 raw copy），
//! 收尾照旧整个换掉。「不产出半成品」「清掉陈旧产物」「最终位置只在收尾这一步被碰到」
//! 三条因此一字没动；归档卷仍旧整包重打，只是留下的成员一个字节都不碰。
//!
//! 页级那条路上整卷跳过还要向上一趟的输出问一句「除了这些再没有别的了吗」
//! （[`Written::holds_nothing_but`]）：页级依据逐页各比各的，看不见源里删掉的那一页
//! 在输出里留下的陈旧产物，而那正是收尾清掉的那一类东西——不问，它就永远留着。

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use zip::write::SimpleFileOptions;

use crate::metadata::{PageRecord, RECORD_PREFIX};
use crate::source::Container;

/// 一个卷的输出容器。写完必须调用 [`Sink::finish`]。
///
/// 两种形态同形：**先写到一个临时容器，收尾时才改名到最终位置**。因此最终位置上
/// 要么是上一趟那一份、要么是这一趟完整的一份，中间那一份不出现——而收尾时
/// **这一卷那几个成员**整个被换掉，输出里于是只剩本趟写出的成员
/// （见 [`DirectorySink`]、[`ArchiveSink`]）。
///
/// 「这一卷那几个成员」多数时候就是「整个去处」，两者分岔只在目录卷的去处里
/// **借住着别的卷**的时候（[`Lodgers`]）。归档卷没有这回事：一个归档是一个文件，
/// 里面住不下另一个卷的去处。
pub enum Sink {
    Directory(DirectorySink),
    /// 装箱是因为 `ZipWriter` 自带几 KB 的压缩状态，直接内嵌会让目录卷也背上这份体积。
    Archive(Box<ArchiveSink>),
}

impl Sink {
    /// 在 `path` 建出输出容器：目录卷是一个目录，归档卷是一个归档文件。
    ///
    /// 这一步建出来的是临时容器，`path` 此刻还没被碰——它要等 [`finish`](Sink::finish)。
    ///
    /// **输出根本身写不写得进不由这里答**：那是这一趟的参数错了，开工前探一次就整趟拒掉了
    /// （见 `crate::ensure_the_output_root_takes_a_write`，06 号票）。到得了这里的失败说的是
    /// **这一卷**这一次建临时容器没建成——这一卷的权限与别处不同、输出根在探过之后才坏、
    /// 盘满——那些是真的「这一卷做不成」，走卷级失败、其余卷照做。
    ///
    /// **最终位置被别的东西占着不在这一句上**：临时容器另占一个名字，建它照样成，
    /// 撞上是在 [`finish`](Sink::finish) 把它改名到位那一步。两处都是卷级失败，
    /// 只是报出来的那句话不同。
    ///
    /// `lodgers` 是**发现**算出来的那一份：`path` 这个去处里还住着哪些别的卷
    /// （见 [`Lodgers`]）。它交在这一句上而不是由调用方先分一次形态：
    /// 「两种形态各要什么」只有这里的 `match` 知道，而归档卷不要它——
    /// 一个归档是一个文件，里面住不下另一个卷的去处。
    pub fn create(path: &Path, container: Container, lodgers: Lodgers) -> Result<Self> {
        match container {
            Container::Directory => Ok(Sink::Directory(DirectorySink::create(path, lodgers)?)),
            Container::Archive => Ok(Sink::Archive(Box::new(ArchiveSink::create(path)?))),
        }
    }

    /// 写一页。页是 PNG，已经 deflate 过一遍，归档里存原样不再压。
    pub fn write_page(&mut self, relative: &Path, bytes: &[u8]) -> Result<()> {
        self.write(relative, bytes, zip::CompressionMethod::Stored)
    }

    /// 写一个透传文件。内容逐字节照搬，只是换个容器。
    pub fn write_extra(&mut self, relative: &Path, bytes: &[u8]) -> Result<()> {
        self.write(relative, bytes, zip::CompressionMethod::Deflated)
    }

    fn write(
        &mut self,
        relative: &Path,
        bytes: &[u8],
        compression: zip::CompressionMethod,
    ) -> Result<()> {
        match self {
            Sink::Directory(directory) => directory.write(relative, bytes),
            Sink::Archive(archive) => archive.write(relative, bytes, compression),
        }
    }

    /// 收尾。两种容器都在这一步才落到最终位置。
    pub fn finish(self) -> Result<()> {
        match self {
            Sink::Directory(directory) => directory.finish(),
            Sink::Archive(archive) => archive.finish(),
        }
    }
}

/// **借住的卷**——一个卷的输出去处里住着的那些**别的卷**，各按相对那个去处的一段
/// 相对路径。这个词是什么、哪种形状才有它，见 `CONTEXT.md` 的《输出》
/// 与 [`DirectorySink`] 的《换的是「这一卷那几个成员」》，那两处不在这里复述。
///
/// 这一份由**发现**那一侧算出来——「这一趟有哪些卷、各镜像到哪儿」只有那里知道
/// （见 `crate::survey::Surveyed::lodgers`）。它相对的是**这一卷的去处**而不是输出根，
/// 干净去处与隔离目录里那个去处因此共用同一份：镜像出来的结构一模一样，
/// 隔离只是在中间插一级。
///
/// 它只答一个问题：[收尾](DirectorySink::finish)时最终位置上哪些东西**不是这一卷的**。
/// 一个都没有就是这一卷独占它的去处——那是目录卷的常态。
#[derive(Debug, Default, Clone)]
pub struct Lodgers(Vec<PathBuf>);

impl Lodgers {
    /// 收下发现那一侧算出来的那几段。
    pub(crate) fn new(inside: Vec<PathBuf>) -> Self {
        Self(inside)
    }

    /// `place` 这个去处里此刻**真住着**别的卷吗。
    ///
    /// 问盘上而不是问名单：名单不空而借住的卷一个都还没写出来时，整个换掉既安全，
    /// 又只有一次改名。
    ///
    /// 答「住着」的有**两种**来路，两种一个待遇——都走窄的那一支，都对：
    ///
    /// - **上一趟留下的那些还在**，常态；
    /// - **本趟已经写出来的那些**。发现多数时候是先序的，混装目录那一卷排在住在它
    ///   里面的那些之前；而收编按「头一回被发现」占座（见 `crate::discover::Found`），
    ///   点名 `N和S/第1话.cbz N和S` 时那一话反而排在**前面**——它先写出来，
    ///   随后混装目录那一卷才收尾。窄的那一支保住它，正是这个次序下唯一对的答案。
    fn any_under(&self, place: &Path) -> bool {
        self.0.iter().any(|inside| place.join(inside).exists())
    }

    /// 去处**直接那一层**的 `name` 是通往某个借住的卷的吗。
    ///
    /// 借住的卷不一定就在直接那一层上（`子目录\第01话.cbz` 也是一个），
    /// 而挡住那一整棵子树只需要认出它的头一级。
    fn leads_to_one(&self, name: &OsStr) -> bool {
        self.0
            .iter()
            .any(|inside| inside.iter().next() == Some(name))
    }

    /// 去处直接那一层的 `name` **有人认领**吗：是这一卷自己的成员（`mine`），或通往某个借住的卷。
    ///
    /// 没人认领的就是陈旧产物。收尾清它（[`DirectorySink::swap_members`]）与整卷跳过之前问
    /// 「还有没有别的」（[`Written::holds_nothing_but`]）用的是**同一句**——两处对「什么算陈旧」
    /// 各说各的，跳过放过的东西收尾就会清掉，或者反过来。
    fn spoken_for(&self, name: &OsStr, mine: &BTreeSet<OsString>) -> bool {
        mine.contains(name) || self.leads_to_one(name)
    }
}

/// 目录输出。
///
/// 先写到一个临时目录，收尾时把最终位置上**这一卷那几个成员**整个换掉。两件事各要它一半：
///
/// 一是**不产出半成品**：写到一半的目录里每一页都带着完全正确的记录，
/// 摆在文件管理器里与一本处理好的书没有分别。
///
/// 二是**清掉陈旧产物**：只覆盖本趟写出的文件的话，源里删掉的那一页会在输出里原地留着、
/// 还带着上一趟的记录，下一趟又被幂等跳过，从此永久留在输出里。
///
/// 归档卷本来就整个重写，两件事在它身上都不成立——同一条标准这才在两种容器形态上
/// 给出同一个答案。
///
/// # 换的是「这一卷那几个成员」，不是「整个去处」
///
/// 混装目录里这两句不是同一句：`out\N和S` 既是封面那一卷的去处，也是
/// `out\N和S\第01话.cbz` 那些兄弟卷输出的父目录（ADR 0014 决定第 1 条：
/// 一个目录可以既是卷又装着卷）。整个换掉就把兄弟卷上一趟的产物一起删了——
/// 同一趟里它们随后照写，末了的盘是对的；而**那一趟没走完**（按了收尾／中止、
/// 某个兄弟卷做不成）时，上一趟好好的产物就再也回不来了（停车场 Q113）。
///
/// 因此这一层认得出[借住的卷](Lodgers)，换掉的范围按它分两支：
///
/// - 去处里此刻**一个借住的卷都没有**——最终位置**整个**换掉
///   （[`swap_the_whole_place`](Self::swap_the_whole_place)）：一次腾位置加一次改名，
///   与从前逐字节相同。头一趟、以及绝大多数目录卷走的都是这一支。
/// - 去处里**住着别的卷**——只换这一卷自己那几个成员
///   （[`swap_members`](Self::swap_members)），借住的那些一个字节都不碰。
///
/// **两件事在窄的那一支上都还在。** 不产出半成品：搬在清之前，那一小段里最终位置上是
/// 「本趟这一份加几件还没清掉的陈旧产物」——是个超集，从不是一本缺页的书；
/// 而最终位置**仍旧只在收尾这一步被碰到**，中途失败与中止照旧一个字节都动不到它。
/// 清掉陈旧产物：直接那一层上凡是不通往借住的卷的东西，本趟没写出它就清掉。
///
/// 两笔跟着这一支长出来的账，各记着一条停车场：
///
/// - **收尾这一步自己没走完时的现场，是宽的那一支产生不出来的那一个**：搬那一步回
///   `Err`（改名撞上权限、盘满），或者进程被硬杀——最终位置上是上一趟那一份**加上**
///   本趟已经搬到位的那几个。它照旧是个超集，下一趟幂等对不上、整卷重做，重做那一趟
///   再清一遍；而宽的那一支在同一刻是「整个去处不存在」。两相比较不算变坏，
///   但它不是一次原子改名（停车场 Q304）。
/// - **借住的卷的过期副本从此留着**：隔离目录里 `out\_isolated\N和S\第01话.cbz` 这样
///   一份——上一趟那一话失败去了隔离，这一趟它做成了、去了干净去处——通往一个借住的卷，
///   清那一步因此放过它。那与「在别处一律不做破坏性动作」是同一条
///   （见 `crate::superseded`）：它是**别人的**过期副本，去留由用户定。
///   从前它由整个换掉顺手删掉（停车场 Q305）。
///
/// **只看直接那一层就够了**：目录卷只收直接那一层（ADR 0014 决定第 1 条），
/// 它写出的成员因此全在那一层上；而那一层上的一个目录必然通往别处——
/// 要么是某个借住的卷，要么是上一趟留下的一棵陈旧的树。
pub struct DirectorySink {
    /// 最终位置。
    path: PathBuf,
    /// 正在写的临时目录。改名到位或析构之后为 `None`。
    partial: Option<PathBuf>,
    /// 这个去处里借住着的那些卷。收尾换掉的范围按它收窄（见本类型的文档）。
    lodgers: Lodgers,
}

impl DirectorySink {
    fn create(path: &Path, lodgers: Lodgers) -> Result<Self> {
        let partial = partial_path(path);
        // 上一趟被硬停在半路、连析构都没跑到的临时目录，这一趟当垃圾清掉：
        // 留着它，里面的陈旧成员会混进本趟的输出。
        //
        // 临时名字是**推得出来**的，因此有一个远角上的代价：同一趟里另有一个卷就叫
        // `<名字>.partial` 时，删掉的是它已经写好的输出。挡这一下要在开工之前把点名的卷
        // 全枚举一遍、比一遍去处，那是预扫要做的事（ADR 0011），不在这一层。
        // 换成随机名字能躲开，但硬停留下的垃圾就再也认不出来、也没有下一趟去清它了。
        if partial.exists() {
            std::fs::remove_dir_all(&partial)
                .with_context(|| format!("清掉残留的临时目录 {}", partial.display()))?;
        }
        std::fs::create_dir_all(&partial)
            .with_context(|| format!("建输出目录 {}", partial.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            partial: Some(partial),
            lodgers,
        })
    }

    fn write(&mut self, relative: &Path, bytes: &[u8]) -> Result<()> {
        let root = self.partial.as_ref().expect("收尾之前临时目录恒在");
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("建输出目录 {}", parent.display()))?;
        }
        std::fs::write(&path, bytes).with_context(|| format!("写 {}", path.display()))
    }

    /// 收尾：最终位置上**这一卷那几个成员**换成这一趟写出的那一份。
    ///
    /// 分岔只有这一处，两支各是什么、为什么，见本类型的《换的是「这一卷那几个成员」》。
    /// 问的是**盘上此刻真住着谁**而不是那份名单：名单不空而兄弟卷一个都还没写出来
    /// （头一趟就是这个样子），整个换掉既安全又只有一次改名，没有理由退到窄的那一支上。
    fn finish(mut self) -> Result<()> {
        let partial = self.partial.clone().expect("收尾之前临时目录恒在");
        if self.lodgers.any_under(&self.path) {
            self.swap_members(&partial)?;
        } else {
            self.swap_the_whole_place(&partial)?;
        }
        // 到这里临时目录已经不在了：整个换掉那一支把它改名走了，只换成员那一支把它清掉了。
        self.partial = None;
        Ok(())
    }

    /// 去处里一个借住的卷都没有：最终位置**整个**换掉。
    ///
    /// 先腾位置再改名，而不是「把旧的改名到一边、放好新的、再删旧的」：多出来的那个
    /// 中间名字要么留在输出里让用户猜，要么还得为「删它又失败了」再写一手。
    /// 腾位置这一步失败就整趟失败，最终位置原样不动，临时目录由析构收走。
    fn swap_the_whole_place(&self, partial: &Path) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_dir_all(&self.path)
                .with_context(|| format!("腾出输出位置 {}", self.path.display()))?;
        }
        std::fs::rename(partial, &self.path)
            .with_context(|| format!("把 {} 改名到 {}", partial.display(), self.path.display()))
    }

    /// 去处里住着别的卷：只换这一卷自己那几个成员。
    ///
    /// 两步，**搬在清之前**：反过来的话那一小段里最终位置上是一本缺页的书，
    /// 而这个次序留下的是一个超集（见本类型的文档）。
    ///
    /// 一、本趟写出的那几个搬到位。搬的是**临时目录直接那一层**，那就是本趟的全部成员。
    /// 二、最终位置直接那一层里，既不是本趟成员、也不通往借住的卷的，清掉——
    ///    源里删掉的那一页正落在这一格上。
    ///
    /// 借住的卷**连搬都不搬**：碰它一下就是从前那条路踩掉的东西。
    fn swap_members(&self, partial: &Path) -> Result<()> {
        let mine = names_in(partial)?;
        for name in &mine {
            let source = partial.join(name);
            let target = self.path.join(name);
            // 同名的目录挡着改名：一个文件改不到一个目录上去。它几乎恒是上一趟留下的
            // 陈旧产物，与同名的陈旧文件同一个待遇；而**极罕见的一种是撞名的借住的卷**
            // ——源里同时有 `001.jpg` 这一页与 `001.png` 这个目录卷时，两者的去处同名，
            // 而撞名那一道只比卷与卷，这一种今天没人查（停车场 Q301）。
            // 两种一个待遇：本趟的成员让它让位，与「整个换掉」那一支的结果逐字节相同。
            if is_a_real_directory(&target) {
                std::fs::remove_dir_all(&target)
                    .with_context(|| format!("腾出输出位置 {}", target.display()))?;
            }
            std::fs::rename(&source, &target)
                .with_context(|| format!("把 {} 改名到 {}", source.display(), target.display()))?;
        }
        for name in names_in(&self.path)? {
            if self.lodgers.spoken_for(&name, &mine) {
                continue;
            }
            let stale = self.path.join(&name);
            remove(&stale).with_context(|| format!("清掉陈旧产物 {}", stale.display()))?;
        }
        // 成员都搬走了，临时目录此刻是空的。
        std::fs::remove_dir_all(partial)
            .with_context(|| format!("清掉临时目录 {}", partial.display()))
    }
}

impl Drop for DirectorySink {
    fn drop(&mut self) {
        // 改名到位就没有临时目录了；还在就是中途出了错，别把半个卷留在输出里。
        if let Some(partial) = self.partial.take() {
            let _ = std::fs::remove_dir_all(partial);
        }
    }
}

/// 上一趟写出的输出容器，只读。幂等要问它三件事：一页里记着什么指纹，一个成员还在不在，
/// 除了点名的这些成员还有没有别的（[`holds_nothing_but`](Self::holds_nothing_but)，two-pass-rework/15）；
/// 按页跳过再向它要第四件——留下的那几页的字节（[`bytes_of`](Self::bytes_of)，two-pass-rework/14）。
///
/// 与 [`Sink`] 对称，也共用同一套容器知识——归档成员名怎么拼只此一份（见 [`archive_name`]）。
/// 两个方向分成两个类型，因为它们的生命期不同：写那一侧要建容器、要收尾，
/// 读这一侧连打开都可能失败，而失败就是「重做」这个平常答案。
///
/// **归档那一支握着一个打开的句柄。**要在这一趟的容器收尾改名**之前**放掉它：
/// 最终位置就是它打开的那个文件，Windows 上开着句柄的文件改不了名。
pub enum Written {
    Directory(PathBuf),
    Archive(Box<zip::ZipArchive<BufReader<File>>>),
}

impl Written {
    /// 打开上一趟的输出。容器根本不在就是 `None`——那是头一趟，没什么可比的。
    pub fn open(path: &Path, container: Container) -> Option<Self> {
        match container {
            Container::Directory => path
                .is_dir()
                .then(|| Written::Directory(path.to_path_buf())),
            Container::Archive => {
                let file = BufReader::new(File::open(path).ok()?);
                Some(Written::Archive(Box::new(zip::ZipArchive::new(file).ok()?)))
            }
        }
    }

    /// 读回一页里记着的那份记录：幂等依据那几项，加上这一张的来路。成员不在、
    /// 或它没有记录，就是 `None`（ADR 0006：读回 tEXt 比对）。
    ///
    /// 只读到第一个 IDAT 为止，一个像素都不解——成本停在这里，跳过一卷才比重做一卷便宜。
    pub fn record_of(&mut self, relative: &Path) -> Option<PageRecord> {
        match self {
            Written::Directory(root) => {
                PageRecord::read(BufReader::new(File::open(root.join(relative)).ok()?))
            }
            Written::Archive(archive) => {
                // 归档成员不能回退寻址，解码器却要得起 `Seek`：先取开头一截到内存里。
                // 只取一截而不是整页，理由见 `RECORD_PREFIX`。
                let mut prefix = Vec::new();
                archive
                    .by_name(&archive_name(relative))
                    .ok()?
                    .take(RECORD_PREFIX)
                    .read_to_end(&mut prefix)
                    .ok()?;
                PageRecord::read(Cursor::new(prefix))
            }
        }
    }

    /// 一个成员的全部字节（two-pass-rework/14：留下的页从这里搬）。
    ///
    /// 与 [`record_of`](Self::record_of) 不同，这一读**整页**：那一头只要开头一截的记录，
    /// 这一头要的正是整页——它要原样写进这一趟的容器。读不出来是这一卷做不成
    /// （比对之后、写出之前上一趟的输出被人动了），不是重做的理由：那时重做也未必对。
    pub fn bytes_of(&mut self, relative: &Path) -> Result<Vec<u8>> {
        match self {
            Written::Directory(root) => {
                let path = root.join(relative);
                std::fs::read(&path).with_context(|| format!("读回上一趟写的 {}", path.display()))
            }
            Written::Archive(archive) => {
                let name = archive_name(relative);
                let mut bytes = Vec::new();
                archive
                    .by_name(&name)
                    .with_context(|| format!("上一趟的输出里找 {name}"))?
                    .read_to_end(&mut bytes)
                    .with_context(|| format!("读回上一趟写的成员 {name}"))?;
                Ok(bytes)
            }
        }
    }

    /// 这个成员还在吗。透传文件不带记录，能问的只有在不在。
    pub fn holds(&mut self, relative: &Path) -> bool {
        match self {
            Written::Directory(root) => root.join(relative).is_file(),
            Written::Archive(archive) => archive.by_name(&archive_name(relative)).is_ok(),
        }
    }

    /// 除了 `members` 这些成员，这个容器里**再没有别的了**吗（two-pass-rework/15）。
    ///
    /// 页级那条路上整卷跳过要问它：页级依据逐页各比各的，源里删掉的那一页没有人替它说
    /// 「我不该在这儿」——它那一族在输出里成了陈旧产物，而卷级那一个数从前顺手盖住了这件事。
    /// 不问这一句，那一族会被静默地留在输出里，下一趟又被跳过，从此永久留着。
    ///
    /// 问的范围与收尾清陈旧产物的**同一句**（[`Lodgers::spoken_for`]，见 [`DirectorySink::swap_members`]）：
    /// 目录卷只看直接那一层（成员全在那一层上），通往[借住的卷](Lodgers)的那些不算——它们是别人的；
    /// 归档卷看全部成员名。列不出来就答「有别的」——那时重做一遍，不会得出错的结论。
    pub fn holds_nothing_but<'a>(
        &mut self,
        members: impl IntoIterator<Item = &'a Path>,
        lodgers: &Lodgers,
    ) -> bool {
        match self {
            Written::Directory(root) => {
                let mine: BTreeSet<OsString> = members
                    .into_iter()
                    .filter_map(|member| member.iter().next().map(OsStr::to_os_string))
                    .collect();
                names_in(root)
                    .is_ok_and(|names| names.iter().all(|name| lodgers.spoken_for(name, &mine)))
            }
            Written::Archive(archive) => {
                let mine: BTreeSet<String> = members.into_iter().map(archive_name).collect();
                archive.file_names().all(|name| mine.contains(name))
            }
        }
    }
}

/// 归档输出。
///
/// 先写到一个临时文件，收尾时才改名到位：中途失败的归档没有中央目录，是打不开的垃圾，
/// 不能让它顶着最终文件名留在输出里。目录卷同形，理由见 [`DirectorySink`]。
pub struct ArchiveSink {
    /// 最终位置。
    path: PathBuf,
    /// 正在写的临时文件。改名到位或析构之后为 `None`——与 [`DirectorySink`] 同一格。
    partial: Option<PathBuf>,
    /// 往临时文件里写的那个写入器。收尾时取走：中央目录一写完它就没用了，
    /// 而临时文件还要等改名，两者因此各占一格。
    writer: Option<zip::ZipWriter<BufWriter<File>>>,
}

impl ArchiveSink {
    fn create(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("建输出目录 {}", parent.display()))?;
        }
        let partial = partial_path(path);
        let file =
            File::create(&partial).with_context(|| format!("建输出归档 {}", partial.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            partial: Some(partial),
            writer: Some(zip::ZipWriter::new(BufWriter::new(file))),
        })
    }

    fn write(
        &mut self,
        relative: &Path,
        bytes: &[u8],
        compression: zip::CompressionMethod,
    ) -> Result<()> {
        let name = archive_name(relative);
        let writer = self.writer.as_mut().expect("收尾之前写入器恒在");
        writer
            .start_file(
                &name,
                SimpleFileOptions::default().compression_method(compression),
            )
            .with_context(|| format!("在 {} 里建成员 {name}", self.path.display()))?;
        writer
            .write_all(bytes)
            .with_context(|| format!("写 {} 的成员 {name}", self.path.display()))
    }

    fn finish(mut self) -> Result<()> {
        let partial = self.partial.clone().expect("收尾之前临时文件恒在");
        self.writer
            .take()
            .expect("收尾之前写入器恒在")
            .finish()
            .with_context(|| format!("收尾 {}", self.path.display()))?
            .into_inner()
            .with_context(|| format!("刷出 {}", self.path.display()))?
            .sync_all()
            .with_context(|| format!("落盘 {}", self.path.display()))?;
        std::fs::rename(&partial, &self.path)
            .with_context(|| format!("把 {} 改名到 {}", partial.display(), self.path.display()))?;
        // 改名成功，临时文件已经不在了。
        self.partial = None;
        Ok(())
    }
}

impl Drop for ArchiveSink {
    fn drop(&mut self) {
        // 改名到位就没有临时文件了；还在就是中途出了错，别把半个归档留在输出里。
        // 先放掉写入器：Windows 上还开着句柄的文件删不掉。
        drop(self.writer.take());
        if let Some(partial) = self.partial.take() {
            let _ = std::fs::remove_file(partial);
        }
    }
}

/// 归档里的成员名：ZIP 规范只认 `/`，Windows 上的 `\` 要换过来。
fn archive_name(relative: &Path) -> String {
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// 一个目录直接那一层的名字，排过序。
///
/// 排序买的是**次序确定**：只换成员那一支要按它搬一遍、再按它清一遍，
/// 而 `read_dir` 的次序由文件系统说了算，出了事两次跑的现场会不一样。
fn names_in(dir: &Path) -> Result<BTreeSet<OsString>> {
    let entries =
        std::fs::read_dir(dir).with_context(|| format!("列出 {} 这一层", dir.display()))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let entry = entry.with_context(|| format!("列出 {} 这一层", dir.display()))?;
        names.insert(entry.file_name());
    }
    Ok(names)
}

/// 这是一个真目录吗——指向目录的符号链接不算。
///
/// 分这一下是因为收掉它们的函数不同：真目录走 `remove_dir_all`，
/// 链接本身是一项，走 `remove_file`（`is_dir` 跟着链接走，光问它会挑错那一个）。
fn is_a_real_directory(path: &Path) -> bool {
    path.is_dir() && !path.is_symlink()
}

/// 清掉最终位置上的一项，是文件是目录都清得掉。
fn remove(path: &Path) -> std::io::Result<()> {
    if is_a_real_directory(path) {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

/// 临时容器的位置：在最终名字后面接一段固定后缀，与最终位置**同一层**，
/// 改名因此不跨卷，也就不会退化成一次逐字节复制。
fn partial_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".partial");
    path.with_file_name(name)
}
