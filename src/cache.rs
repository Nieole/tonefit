//! 卷缓存：第一遍存下缩放到目标尺寸的参照，第二遍取回（ADR 0005：解码一次，缓存缩放后的图）。
//!
//! 存的是参照本身——判据算过的那张 8 位灰度图。第二遍要做的量化与编码只吃它，
//! 源页因此在第一遍之后再没人碰。
//!
//! 内存优先，超出预算的那些页溢写临时文件。**缓存只活在一次运行之内**：
//! 进程结束即释放内存、收走临时文件，不提供跨运行的持久缓存。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use crate::geometry::Size;
use crate::gray::GrayImage;

/// 缓存预算（`--cache-budget`）：缓存最多在内存里留这么多字节。
///
/// 量的是**压缩之后**的字节——那才是真正占着内存的那个数。超出的页不被丢弃，而是溢写临时文件，
/// 因此预算限的是峰值内存，不是卷的大小上限（spec 的 story 31）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheBudget(u64);

/// 不点名时的预算：512 MiB。
///
/// 按 ADR 0005 的缓存体积估算，它够装下几百页的卷而不落到溢写路径上。那个估算取的是
/// 未压缩的上界，而真实片源上 LZ4 能压到多少，ADR 里写着**尚未测量**——
/// 这个默认值因此按上界取，偏保守。
const DEFAULT_BUDGET: u64 = 512 * 1024 * 1024;

impl CacheBudget {
    /// 按字节数造一个预算。
    pub fn new(bytes: u64) -> Self {
        Self(bytes)
    }

    /// 按 `--cache-budget` 的写法解析：纯字节数，或带 K/M/G 后缀（`512M`、`2G`）。
    ///
    /// 后缀按 1024 进位，大小写不论，`B` 与 `iB` 两种尾巴都收。取值集合不进 CLI 的类型，
    /// 库这一侧对 CLI 无知——与 `--filter`、`--bit-depth` 同一套分工。
    pub fn parse(text: &str) -> Result<Self> {
        let lowered = text.trim().to_ascii_lowercase();
        let digits = lowered.trim_end_matches(|c: char| !c.is_ascii_digit());
        let suffix = &lowered[digits.len()..];
        let scale = match suffix.trim_end_matches('b').trim_end_matches('i') {
            "" => 1,
            "k" => 1024,
            "m" => 1024 * 1024,
            "g" => 1024 * 1024 * 1024,
            _ => bail!("认不出缓存预算 {text} 的单位：后缀写 K、M 或 G"),
        };
        let Ok(count) = digits.parse::<u64>() else {
            bail!("认不出缓存预算 {text}：写成字节数，或带 K/M/G 后缀，例如 512M");
        };
        match count.checked_mul(scale) {
            Some(bytes) => Ok(Self(bytes)),
            None => bail!("缓存预算 {text} 大得装不进 64 位字节数"),
        }
    }

    /// 预算的字节数。
    pub fn bytes(self) -> u64 {
        self.0
    }
}

impl Default for CacheBudget {
    fn default() -> Self {
        Self(DEFAULT_BUDGET)
    }
}

impl std::fmt::Display for CacheBudget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format_bytes(self.0))
    }
}

/// 一个卷的缓存用量。
///
/// 卷成为不可分割的处理单元，峰值内存随卷大小线性增长——这是 ADR 0005 认下的代价，
/// 报告里说出用量与是否溢写，是这条代价对用户唯一可见的形式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheUsage {
    /// 本次的预算。
    pub budget: CacheBudget,
    /// 缓存里的页数。透传文件不进缓存。
    pub pages: usize,
    /// 这些页摊开来有多少字节：目标尺寸 × 每像素一字节。
    pub raw: u64,
    /// LZ4 压过之后实际存下多少字节。恒等于 `resident + spilled`。
    pub stored: u64,
    /// 其中留在内存里的字节。
    pub resident: u64,
    /// 其中溢写到临时文件的字节。为零即本次没有溢写。
    pub spilled: u64,
}

impl CacheUsage {
    /// 一份什么都没存的用量。幂等命中而跳过的卷报的就是它——那一趟一页都没进缓存。
    pub(crate) fn new(budget: CacheBudget) -> Self {
        Self {
            budget,
            pages: 0,
            raw: 0,
            stored: 0,
            resident: 0,
            spilled: 0,
        }
    }
}

impl std::fmt::Display for CacheUsage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} 页 {}（压缩前 {}）",
            self.pages,
            format_bytes(self.stored),
            format_bytes(self.raw)
        )?;
        if self.spilled == 0 {
            write!(f, "，未溢写（预算 {}）", self.budget)
        } else {
            write!(
                f,
                "，内存 {} + 临时文件 {}（预算 {}）",
                format_bytes(self.resident),
                format_bytes(self.spilled),
                self.budget
            )
        }
    }
}

/// 这一遍的缓存留不留页。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retention {
    /// 留下：第二遍要从这里把页取回来。
    Keep,
    /// 只记账。dry-run 没有第二遍，留下的字节没人取——但用量与会不会溢写仍要预告
    /// （spec 的 story 6：先看一份报告再决定照不照做，`--cache-budget` 正是要确认的参数之一）。
    /// 因此照压、照按预算算账，只是不留下块，也就**不建临时文件**——
    /// dry-run 那句「不写输出」在命令行这一路是连临时文件都不建，峰值内存也不为一次预演白占。
    Account,
}

/// 一个卷的缓存。存入顺序就是取出用的序号。
pub struct PageCache {
    retention: Retention,
    /// 溢写文件建在哪个目录下。
    spill_dir: PathBuf,
    entries: Vec<Entry>,
    /// 溢写文件。头一页装不下时才建，全程留在内存里的卷根本不碰文件系统。
    spill: Option<Spill>,
    usage: CacheUsage,
}

/// 一页压好的块，等着进缓存。
///
/// 压缩与存放分成两步，为的是计算层那一侧：压缩是**纯计算**、每页各做各的，
/// 存放要动共用的账本、非串起来不可。合成一步，那把锁就得连压缩一起罩住——
/// 满核跑起来时它会成为唯一的瓶颈，而压缩恰恰是这两件事里贵的那一件。
pub struct Block {
    size: Size,
    /// LZ4 压过的像素。
    block: Vec<u8>,
    /// 摊开来有多少字节。用量那一行里的「压缩前」就是它。
    raw: u64,
}

/// 把一页压成一个待存的块。不碰缓存，因此并发做没有争用。
pub fn compress(image: &GrayImage) -> Block {
    Block {
        size: image.size(),
        block: lz4_flex::compress(image.pixels()),
        raw: image.pixels().len() as u64,
    }
}

/// 一页在缓存里的样子：尺寸，加上 LZ4 压过的像素在哪儿。
///
/// 尺寸单独留着而不是从压缩块里读——解压要先知道摊开有多大。
struct Entry {
    size: Size,
    stored: Stored,
}

/// 一页的压缩块待在哪里。
enum Stored {
    Memory(Vec<u8>),
    Spilled(Slot),
    /// 只量过大小，块没留下（[`Retention::Account`]）。
    Measured,
}

/// 溢写文件里的一段。
#[derive(Debug, Clone, Copy)]
struct Slot {
    offset: u64,
    len: usize,
}

impl PageCache {
    /// 建一个卷缓存，溢写落在系统临时目录下。
    pub fn new(budget: CacheBudget, retention: Retention) -> Self {
        Self::spilling_into(budget, retention, std::env::temp_dir())
    }

    /// 同上，但点名溢写目录。
    fn spilling_into(
        budget: CacheBudget,
        retention: Retention,
        spill_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            retention,
            spill_dir: spill_dir.into(),
            entries: Vec::new(),
            spill: None,
            usage: CacheUsage::new(budget),
        }
    }

    /// 至此存下了多少。
    pub fn usage(&self) -> CacheUsage {
        self.usage
    }

    /// 把一个压好的块存进来，返回它的序号。
    ///
    /// 内存优先：预算还装得下就留在内存里，装不下的那一页溢写（ADR 0005）。
    /// 判的是这一页放进去之后会不会越过预算，因此单页大过整个预算时它自己溢写，
    /// 已经在内存里的那些页不受牵连。
    ///
    /// **谁常驻、谁溢写随存入顺序而变**，而第一遍是乱序满核跑的（13 号票）：同一个卷、
    /// 同一份预算，两趟跑出来的 `resident` 与 `spilled` 分法可能不同。总量不受影响——
    /// `pages`、`raw`、`stored` 与顺序无关，写出的字节也一个不差，分法只改这一页
    /// 待在内存里还是临时文件里。认下它是因为另一头更贵：要让分法确定，存入就得按页序串起来，
    /// 而那正是计算层满核要拆掉的东西。
    pub fn insert(&mut self, page: Block) -> Result<usize> {
        let Block { size, block, raw } = page;
        let length = block.len() as u64;
        let resident = self.usage.resident + length <= self.usage.budget.bytes();
        // 先落定，后记账：溢写写盘失败时，用量不该记下一页其实并不存在的缓存。
        let stored = match (self.retention, resident) {
            (Retention::Account, _) => Stored::Measured,
            (Retention::Keep, true) => Stored::Memory(block),
            (Retention::Keep, false) => Stored::Spilled(self.spill()?.append(&block)?),
        };
        self.usage.pages += 1;
        self.usage.raw += raw;
        self.usage.stored += length;
        if resident {
            self.usage.resident += length;
        } else {
            self.usage.spilled += length;
        }
        self.entries.push(Entry { size, stored });
        Ok(self.entries.len() - 1)
    }

    /// 取回一页。
    pub fn load(&mut self, index: usize) -> Result<GrayImage> {
        let Some(entry) = self.entries.get(index) else {
            bail!(
                "缓存里没有第 {index} 页：只存下了 {} 页",
                self.entries.len()
            );
        };
        let size = entry.size;
        let pixels = (size.width as usize) * (size.height as usize);
        let slot = match &entry.stored {
            Stored::Memory(block) => return unpack(block, size, pixels, index),
            Stored::Spilled(slot) => *slot,
            Stored::Measured => bail!(
                "这一遍的缓存只记账、不留页：第 {index} 页取不回来。\
                 dry-run 没有第二遍，取页说明调用方走错了路"
            ),
        };
        let spill = self.spill.as_mut().expect("有溢写记录就有溢写文件");
        let block = spill
            .read(slot)
            .with_context(|| format!("从溢写文件读缓存里第 {index} 页"))?;
        unpack(&block, size, pixels, index)
    }

    /// 溢写文件，没有就现建一个。
    fn spill(&mut self) -> Result<&mut Spill> {
        if self.spill.is_none() {
            self.spill = Some(Spill::create(&self.spill_dir)?);
        }
        Ok(self.spill.as_mut().expect("刚建过"))
    }
}

/// 把一个压缩块摊回一页。
fn unpack(block: &[u8], size: Size, pixels: usize, index: usize) -> Result<GrayImage> {
    let restored =
        lz4_flex::decompress(block, pixels).with_context(|| format!("解出缓存里第 {index} 页"))?;
    // `decompress` 收的是「至多这么大」：块坏掉时它给回短一截的缓冲而不是报错，
    // 而 `GrayImage::new` 只有 debug_assert 拦着——release 下那截短缓冲会一路流进量化与编码。
    // 这一句是它唯一的守卫。
    ensure!(
        restored.len() == pixels,
        "缓存里第 {index} 页解出 {} 字节，该有 {pixels} 字节",
        restored.len()
    );
    Ok(GrayImage::new(size, restored))
}

/// 溢写的去处：一个匿名临时文件。
///
/// 它从建出来就没有可打开的名字——Unix 上立刻 unlink，Windows 上带 delete-on-close 标志，
/// 两边都由操作系统在最后一个句柄关闭时收走。进程被中断也一样：句柄由内核关，
/// 因此**中断不留下孤儿临时文件**，不必靠下次运行去扫。
struct Spill {
    file: File,
    /// 已写到哪儿。追加的下一段从这里开始。
    end: u64,
}

impl Spill {
    fn create(dir: &Path) -> Result<Self> {
        let file = tempfile::tempfile_in(dir)
            .with_context(|| format!("在 {} 下建缓存的溢写文件", dir.display()))?;
        Ok(Self { file, end: 0 })
    }

    fn append(&mut self, block: &[u8]) -> Result<Slot> {
        self.file
            .seek(SeekFrom::Start(self.end))
            .context("定位到溢写文件末尾")?;
        self.file.write_all(block).context("写溢写文件")?;
        let slot = Slot {
            offset: self.end,
            len: block.len(),
        };
        self.end += block.len() as u64;
        Ok(slot)
    }

    fn read(&mut self, slot: Slot) -> Result<Vec<u8>> {
        self.file
            .seek(SeekFrom::Start(slot.offset))
            .context("定位到溢写文件里的那一段")?;
        let mut block = vec![0; slot.len];
        self.file.read_exact(&mut block).context("读溢写文件")?;
        Ok(block)
    }
}

/// 字节数的人话形态。
///
/// 进位按 1024，单位就标 KiB/MiB/GiB——`--cache-budget` 那一侧也按 1024 收，
/// 标成 KB/MB/GB 会让报告里的数与用户敲进去的那个数不是同一个量。
fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一页能压缩、但压不成一个字节的图：横向斜坡加一点纵向偏移。
    fn page(size: Size) -> GrayImage {
        let pixels = (0..size.height)
            .flat_map(|y| (0..size.width).map(move |x| ((x * 7 + y * 13) % 251) as u8))
            .collect();
        GrayImage::new(size, pixels)
    }

    /// 存进去的与取出来的逐字节相同——内存与溢写两条路都是。
    /// LZ4 无损是「第二遍不改变任何输出」的前提。
    #[test]
    fn what_goes_in_comes_back_out_byte_for_byte_wherever_it_was_kept() {
        let spill_dir = tempfile::tempdir().expect("建溢写目录");
        for budget in [CacheBudget::default(), CacheBudget::new(0)] {
            let image = page(Size::new(64, 48));
            let mut cache = PageCache::spilling_into(budget, Retention::Keep, spill_dir.path());

            let index = cache.insert(compress(&image)).expect("存一页");
            let restored = cache.load(index).expect("取回那一页");

            assert_eq!(restored.size(), image.size(), "预算 {budget}");
            assert_eq!(restored.pixels(), image.pixels(), "预算 {budget}");
        }
    }

    /// 预算是内存的界：装得下的留在内存，装不下的溢写，而两边都取得回来。
    ///
    /// 预算按「第一页存下之后的常驻量」定，因此第一页恰好装满、第二页起溢写——
    /// 界的两侧各有一页，这条才既量得到内存优先，也量得到溢写。
    #[test]
    fn the_budget_decides_who_stays_in_memory_and_who_spills() {
        let spill_dir = tempfile::tempdir().expect("建溢写目录");
        let pages: Vec<_> = [40u32, 48, 56]
            .iter()
            .map(|&h| page(Size::new(64, h)))
            .collect();

        let mut measured =
            PageCache::spilling_into(CacheBudget::default(), Retention::Keep, spill_dir.path());
        measured
            .insert(compress(&pages[0]))
            .expect("量第一页压完有多大");
        let budget = CacheBudget::new(measured.usage().stored);

        let mut cache = PageCache::spilling_into(budget, Retention::Keep, spill_dir.path());
        for image in &pages {
            cache.insert(compress(image)).expect("存一页");
        }

        let usage = cache.usage();
        assert_eq!(usage.pages, 3);
        assert_eq!(usage.resident, budget.bytes(), "第一页该正好装满预算");
        assert_eq!(usage.stored, usage.resident + usage.spilled);
        assert!(usage.spilled > 0, "越过预算的页没有溢写");
        assert_eq!(usage.raw, 64 * (40 + 48 + 56));
        // 界的两侧各取回一页：内存那条路与溢写那条路给出的是同一张图。
        for (index, image) in pages.iter().enumerate() {
            let restored = cache.load(index).expect("取回一页");
            assert_eq!(restored.pixels(), image.pixels(), "第 {index} 页");
        }
    }

    /// 溢写文件不在目录里留下孤儿：缓存一放手，那个目录就是空的。
    ///
    /// 中断时的那一半靠的是同一件事——文件从建出来就是匿名/delete-on-close 的，
    /// 收走它的是操作系统而不是这里的析构（见 [`Spill`]）。
    #[test]
    fn a_spilled_cache_leaves_no_file_behind() {
        let spill_dir = tempfile::tempdir().expect("建溢写目录");
        let mut cache =
            PageCache::spilling_into(CacheBudget::new(0), Retention::Keep, spill_dir.path());
        cache
            .insert(compress(&page(Size::new(64, 48))))
            .expect("存一页");
        assert!(cache.usage().spilled > 0, "这一页没有溢写，下面就白测了");
        // Unix 上文件已经 unlink，此刻目录就已经是空的；Windows 上要等句柄关闭。
        #[cfg(unix)]
        assert_eq!(spill_dir.path().read_dir().expect("读溢写目录").count(), 0);

        drop(cache);

        assert_eq!(
            spill_dir.path().read_dir().expect("读溢写目录").count(),
            0,
            "溢写文件留在了 {}",
            spill_dir.path().display()
        );
    }

    /// 只记账那一遍：用量照记，页不留，临时文件一个不建。
    ///
    /// dry-run 走的就是这条路——它没有第二遍，留下的字节没人取，但 `--cache-budget`
    /// 撑不撑得住仍要预告得出来（spec 的 story 6）。
    #[test]
    fn an_accounting_only_cache_measures_everything_and_keeps_nothing() {
        let spill_dir = tempfile::tempdir().expect("建溢写目录");
        let image = page(Size::new(64, 48));
        // 预算为零：真留页的那一遍会在这里溢写，只记账的这一遍连文件都不该建。
        let mut cache =
            PageCache::spilling_into(CacheBudget::new(0), Retention::Account, spill_dir.path());

        let index = cache.insert(compress(&image)).expect("量一页");

        let usage = cache.usage();
        assert_eq!(usage.pages, 1);
        assert_eq!(usage.raw, 64 * 48);
        assert!(usage.stored > 0, "没有量出压缩后的大小");
        // 预告说得出「照做的话这一页会溢写」，而此刻并没有真溢写。
        assert_eq!(usage.spilled, usage.stored);
        assert_eq!(
            spill_dir.path().read_dir().expect("读溢写目录").count(),
            0,
            "只记账的一遍建出了临时文件"
        );
        // 页真的没留下：来取就是调用方走错了路，不是悄悄给一张空图。
        assert!(cache.load(index).is_err(), "只记账的一遍竟取回了页");
    }

    /// 记账与真留页给出同一份用量——dry-run 的预告因此与照做时对得上。
    #[test]
    fn accounting_only_and_keeping_agree_on_the_usage() {
        let spill_dir = tempfile::tempdir().expect("建溢写目录");
        let pages: Vec<_> = [40u32, 48, 56]
            .iter()
            .map(|&h| page(Size::new(64, h)))
            .collect();
        let budget = CacheBudget::new(4096);

        let usage = |retention| {
            let mut cache = PageCache::spilling_into(budget, retention, spill_dir.path());
            for image in &pages {
                cache.insert(compress(image)).expect("存一页");
            }
            cache.usage()
        };

        assert_eq!(usage(Retention::Account), usage(Retention::Keep));
    }

    /// 缓存里的块坏掉时当场报错，不把短一截的缓冲交出去。
    ///
    /// `lz4_flex::decompress` 收的是「至多这么大」：块坏了它给回短缓冲而不是报错，
    /// 而 `GrayImage::new` 只有 debug_assert 拦着——这一条守的是 release 下那条路。
    #[test]
    fn a_short_block_is_refused_rather_than_handed_on() {
        let size = Size::new(64, 48);
        let pixels = (size.width * size.height) as usize;
        // 一个只解得出半页的合法 LZ4 块。
        let half = lz4_flex::compress(&vec![7u8; pixels / 2]);

        let error = unpack(&half, size, pixels, 0).expect_err("短一截的块应当被拒");

        assert!(error.to_string().contains("该有"), "{error}");
    }

    /// 预算的写法：纯字节数，或带 K/M/G 后缀。
    #[test]
    fn a_budget_reads_as_bytes_or_with_a_unit() {
        let bytes = |text: &str| CacheBudget::parse(text).expect(text).bytes();
        assert_eq!(bytes("0"), 0);
        assert_eq!(bytes("4096"), 4096);
        assert_eq!(bytes("512M"), 512 * 1024 * 1024);
        // 大小写不论，B 与 iB 两种尾巴都收。
        assert_eq!(bytes("512m"), bytes("512MB"));
        assert_eq!(bytes("512MiB"), bytes("512M"));
        assert_eq!(bytes("2g"), 2 * 1024 * 1024 * 1024);
        assert_eq!(bytes("8k"), 8 * 1024);
        // 认不出的写法当场被挡下，不是一个待猜的数。
        for text in ["", "M", "512T", "1.5G", "-1", "512 M B"] {
            assert!(CacheBudget::parse(text).is_err(), "{text} 不该解析出预算");
        }
    }

    /// 用量那一行要说清两件事：占了多少，以及有没有溢写。
    #[test]
    fn the_usage_line_says_how_much_and_whether_it_spilled() {
        let mut usage = CacheUsage::new(CacheBudget::parse("512M").expect("512M"));
        usage.pages = 2;
        usage.raw = 4 * 1024 * 1024;
        usage.stored = 1024 * 1024;
        usage.resident = usage.stored;
        let said = usage.to_string();
        assert!(said.contains("2 页"), "{said}");
        assert!(said.contains("1.0 MiB"), "{said}");
        assert!(said.contains("压缩前 4.0 MiB"), "{said}");
        assert!(said.contains("未溢写"), "{said}");
        // 进位按 1024，单位就得标 MiB：`--cache-budget 512M` 收的正是这个量。
        assert!(said.contains("预算 512.0 MiB"), "{said}");

        usage.resident = 0;
        usage.spilled = usage.stored;
        let said = usage.to_string();
        assert!(said.contains("临时文件 1.0 MiB"), "{said}");
        assert!(!said.contains("未溢写"), "{said}");
    }
}
