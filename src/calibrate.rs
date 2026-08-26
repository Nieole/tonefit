//! 标定图：一次上机同时回答两件事。
//!
//! 1. **像素有没有原样贴上**——抖动块与同均值实心块分不分得开，1 像素周期光栅有没有纹理，
//!    四角标记在不在。这一件不成立时抖动做了等于没做（measurements 的《真机像素完整性》），
//!    而 tonefit 探不到它：阅读器的显示管线在视野之外（ADR 0007 的备选方案）。
//! 2. **还分得开几级灰**——最右那条阶梯数出来的数回填给 `--gray-levels`（ADR 0003）。
//!
//! **两件事有先后**，先后印在图内：缩放没关掉时阶梯本身就被重采样过，数出来的不是面板
//! 能显示的级数。因此第一件不通过就别数第二件。两件事合在一张图上，是因为它们只有
//! **一次上机**才有价值——分成两张，第二张就没人拷进设备了。

mod glyphs;

use std::path::Path;

use anyhow::{Context, Result};

use crate::encode;
use crate::geometry::Size;
use crate::gray::GrayImage;
use crate::profile::Profile;
use crate::quantize::{BitDepth, grid_level};
use glyphs::{FULL_WIDTH, GLYPH_HEIGHT, HALF_WIDTH};

/// 纸白。图的底色，也是阶梯最亮的那一档。
const PAPER: u8 = 255;

/// 墨黑。判读说明、边框与四角标记的颜色，也是阶梯最暗的那一档。
const INK: u8 = 0;

/// 画一张标定图并写到 `out`，父目录不在就建出来。
///
/// 库的第三个 seam 落在这一条上，契约见 [`crate::write_calibration_chart`]。
///
/// 写法是最朴素的那一种：建目录、写文件，不走临时文件加改名那一套。
/// 输出容器要那套是因为**一卷做到一半的目录冒充得了做完的**，下一趟幂等会当它齐了；
/// 标定图没有那个角色——没有哪一趟程序会回来读它并当它齐了，
/// 而人读它是在设备上打开，写坏了一眼就看得见，重敲一次命令即可。
pub fn write_chart(profile: &Profile, out: &Path) -> Result<()> {
    let bytes = chart_png(profile)?;
    // 点名的是个裸文件名时 `parent()` 给的是空串，那时没有目录要建。
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("建标定图的去处 {}", parent.display()))?;
    }
    std::fs::write(out, &bytes).with_context(|| format!("写标定图 {}", out.display()))
}

/// 画一张标定图，编成 PNG 字节。
///
/// 8 位工作精度、不带记录，两样都是定死的，理由见 [`crate::write_calibration_chart`]。
fn chart_png(profile: &Profile) -> Result<Vec<u8>> {
    encode::png(&chart(profile), BitDepth::Eight, None)
}

/// 按目标 profile 画一张标定图，尺寸恒等于面板分辨率。
fn chart(profile: &Profile) -> GrayImage {
    let layout = Layout::plan(profile);
    let mut canvas = Canvas::new(profile.panel().resolution, PAPER);

    for line in &layout.text {
        canvas.text(line.left, line.top, line.scale, INK, &line.text);
    }
    for pair in &layout.pairs {
        pair.draw(&mut canvas, layout.hairline);
    }
    for grating in &layout.gratings {
        grating.draw(&mut canvas, layout.hairline);
    }
    for ladder in &layout.ladders {
        ladder.draw(&mut canvas, &layout);
    }
    // 四角标记最后画：它压在第 0 行列与末行列上，谁都不该盖住它。
    layout.corners(&mut canvas);
    canvas.into_image()
}

/// 一张标定图的版式：哪一行字排在哪儿，两排方块与各条阶梯各占哪一块。
///
/// 尺寸一概按面板算、不写死像素：面板从 824 宽到 1860 宽，同一个常数在两头一个嫌挤一个嫌空。
/// 整份版式一次算完，画的时候只管照着填——版式与作画分开，两边才不会各算一遍而算得不一样。
struct Layout {
    /// 边框与细线的粗细。300 PPI 上一像素的线细到看不见。
    hairline: u32,
    /// 四角标记的臂长。
    arm: u32,
    /// 阶梯抬头的字号。它按**栏宽**定，不跟着说明走——抬头得在自己那一栏里放得下。
    header_scale: u32,
    /// 印在图上的每一行字，连同它的落点与字号。
    text: Vec<TextLine>,
    /// 抖动块与同均值实心块的并置，一档灰度一对。
    pairs: Vec<Pair>,
    /// 1 像素周期光栅，四种。
    gratings: Vec<Grating>,
    /// 各条阶梯，从左到右。
    ladders: Vec<Ladder>,
}

impl Layout {
    fn plan(profile: &Profile) -> Self {
        let panel = profile.panel();
        let resolution = panel.resolution;
        let margin = (resolution.width / 24).max(8);
        let hairline = (resolution.width / 400).max(1);
        let content = resolution.width - margin * 2;
        let available = resolution.height - margin * 2;

        // 位深按面板灰阶数裁（ADR 0003）：图排的是**这台设备真会用到的**那几档，不是位深全集。
        let depths = BitDepth::candidates(panel.gray_levels);
        let columns = depths.len() as u32;
        let column = (content - margin * (columns - 1)) / columns;
        let legend = legend(profile);
        // 两排方块各占面板高的十四分之一：再矮就看不出抖动块那点颗粒，再高就该轮到阶梯抱怨了。
        let patch = resolution.height / 14;
        let scale = legend.scale(content, available, patch);
        // 抬头共用一个字号：各栏字号不一，眼睛会把它读成「这一条更要紧」。
        // 它也不许大过说明——抬头压过判读说明，读的人会先去数阶梯。
        let header_scale = depths
            .iter()
            .map(|&depth| fitting_scale(&header(depth), column))
            .min()
            .expect("候选位深至少有 1bit 那一档")
            .min(scale);

        // 自上而下：方块上面那几组说明、两排方块、方块下面那一组说明，抬头之下剩的全归阶梯。
        // **每一节的说明排在它那几块的上面**——说明在下面的话，人会先看见方块、后知道该看什么。
        let gap = line_height(scale) / 2;
        let mut text = Vec::new();
        let mut y = margin;
        for group in legend.above() {
            y = place(&mut text, group, margin, y, scale) + gap;
        }
        let pairs = Pair::row(Rect::new(margin, y, content, patch), margin);
        y += patch + gap;
        let gratings = Grating::row(Rect::new(margin, y, content, patch), margin);
        y += patch + gap;
        y = place(&mut text, legend.below(), margin, y, scale) + gap;

        let top = y + line_height(header_scale);
        let height = resolution.height.saturating_sub(margin + top);
        let ladders = depths
            .iter()
            .enumerate()
            .map(|(index, &depth)| Ladder {
                depth,
                rect: Rect::new(
                    margin + index as u32 * (column + margin),
                    top,
                    column,
                    height,
                ),
            })
            .collect();

        Self {
            hairline,
            arm: (resolution.width / 12).max(16),
            header_scale,
            text,
            pairs,
            gratings,
            ladders,
        }
    }

    /// 四角各压一个直角标记，两条臂分别落在第 0 行列与末行列上。
    ///
    /// 它答的是**边距与裁切**：阅读器自己加了边距、或者裁掉了白边，第一行第一列就不在屏上了，
    /// 少一个角就看得出来。臂要够长——只点一个像素的话，那一个像素落在屏边上没人分得清它在不在。
    fn corners(&self, canvas: &mut Canvas) {
        let Size { width, height } = canvas.size;
        // 比细线粗一倍：标记是拿来一眼扫过去数的，细到与阶梯的边框同粗就要凑近了看。
        let thickness = (self.hairline * 2).max(3);
        let (arm, right, bottom) = (self.arm, width - self.arm, height - thickness);
        for (left, top) in [(0, 0), (right, 0), (0, bottom), (right, bottom)] {
            canvas.fill(Rect::new(left, top, arm, thickness), INK);
        }
        let (right, bottom) = (width - thickness, height - arm);
        for (left, top) in [(0, 0), (right, 0), (0, bottom), (right, bottom)] {
            canvas.fill(Rect::new(left, top, thickness, arm), INK);
        }
    }
}

/// 把一组说明逐行摆下去，回下一组该从哪儿起。
fn place(text: &mut Vec<TextLine>, group: &[String], left: u32, top: u32, scale: u32) -> u32 {
    let mut y = top;
    for line in group {
        text.push(TextLine {
            left,
            top: y,
            scale,
            text: line.clone(),
        });
        y += line_height(scale);
    }
    y
}

/// 印在图上的一行字：落点、字号与内容。
struct TextLine {
    left: u32,
    top: u32,
    scale: u32,
    text: String,
}

/// `count` 块等宽的东西横排在 `band` 里、块与块之间空 `gap` 时，一块最宽能有多宽。
fn spread_width(band: Rect, count: u32, gap: u32) -> u32 {
    (band.size.width - gap * (count - 1)) / count
}

/// 把 `count` 块宽 `width` 的东西横排在 `band` 里，整排横向居中。
///
/// 宽度由调用方给而不是这里算：抖动块那一排要它是 15 的整数倍（均值严格相等的前提），
/// 光栅那一排不要，两者只在这一个数上不同。居中是因为取整会剩下几个像素，
/// 全堆在右边看得出来。
fn spread(band: Rect, count: u32, gap: u32, width: u32) -> Vec<Rect> {
    let inset = (band.size.width - (width * count + gap * (count - 1))) / 2;
    (0..count)
        .map(|index| {
            Rect::new(
                band.left + inset + index * (width + gap),
                band.top,
                width,
                band.size.height,
            )
        })
        .collect()
}

/// 一对方块：左边抖动、右边实心，**两块的均值严格相等**。
///
/// 这是整张图的命门。均值不相等的话，阅读器重采样过之后两块仍然分得开，
/// 判读会给出一个假的「通过」——那比没有这张图更糟。
///
/// 相等靠构造保证，不靠事后凑：抖动块每 15 格里恰好 `level` 格纸白，
/// 而 255 = 15 × 17，一行的均值因此恰好是 `17 × level`——整数，且正好落在 16 级面板的格点上。
/// 块宽取 15 的整数倍，每一行都恰好走完整数个周期，整块的均值于是与逐行的均值同一个数。
/// 实心块填的就是那个数（见 [`Pair::solid_level`]）。
struct Pair {
    /// 抖动的那一半。
    dither: Rect,
    /// 实心的那一半，紧挨着抖动那一半，尺寸相同。
    solid: Rect,
    /// 每 15 格里几格纸白。均值是它的 17 倍。
    level: u32,
}

impl Pair {
    /// 一排四对，横排在 `band` 这一条带里。
    fn row(band: Rect, gap: u32) -> Vec<Self> {
        let count = PAIR_LEVELS.len() as u32;
        // 半块宽取 15 的整数倍——均值严格相等靠的就是每一行走完整数个周期。
        let half = spread_width(band, count, gap) / 2 / DITHER_PERIOD * DITHER_PERIOD;
        spread(band, count, gap, half * 2)
            .into_iter()
            .zip(PAIR_LEVELS)
            .map(|(cell, level)| Self {
                dither: Rect::new(cell.left, cell.top, half, cell.size.height),
                solid: Rect::new(cell.left + half, cell.top, half, cell.size.height),
                level,
            })
            .collect()
    }

    /// 实心那一半的灰：抖动那一半的均值，取整数、且落在 16 级面板的格点上。
    fn solid_level(&self) -> u8 {
        (u32::from(PAPER) * self.level / DITHER_PERIOD) as u8
    }

    /// 画这一对，再沿两块**外沿**围一圈边。
    ///
    /// 边围在外面而不是压在块上：压上去就改掉了块里的像素，均值跟着不再相等，
    /// 而那正是这一对方块唯一要保住的性质。
    fn draw(&self, canvas: &mut Canvas, hairline: u32) {
        let level = self.level;
        canvas.paint(
            self.dither,
            |x, y| {
                if dithered(x, y, level) { PAPER } else { INK }
            },
        );
        canvas.fill(self.solid, self.solid_level());
        canvas.outline(
            Rect::new(
                self.dither.left,
                self.dither.top,
                self.dither.size.width + self.solid.size.width,
                self.dither.size.height,
            ),
            hairline,
        );
    }
}

/// 四对方块取的那几档灰：每 15 格里几格纸白。均值依次是 51、102、153、204，
/// 也就是 16 级面板的第 3、6、9、12 级——四档摊开在整个灰度区间上，不挤在中间。
const PAIR_LEVELS: [u32; 4] = [3, 6, 9, 12];

/// 抖动块的行内周期。255 = 15 × 17，取 15 才使「每 15 格 n 格纸白」的均值恰好是整数 17n。
const DITHER_PERIOD: u32 = 15;

/// 行内的散布步长。与 15 互素，因此每 15 格里恰好取到 `level` 格，而取到的那几格互相散得开。
const DITHER_STRIDE: u32 = 4;

/// 行与行之间的错位，0..15 的一个排列。
///
/// 每一行的图案都是头一行平移出来的，而平移量互不相同：不这么错开的话，
/// 各行同相位，抖动块会变成一片竖条纹——那就成了光栅那一节的东西，而不是抖动。
const ROW_SHIFT: [u32; DITHER_PERIOD as usize] = [0, 6, 11, 3, 13, 8, 1, 10, 4, 14, 7, 2, 12, 5, 9];

/// 抖动块上 `(x, y)` 那一格是不是纸白。
fn dithered(x: u32, y: u32, level: u32) -> bool {
    let shift = ROW_SHIFT[(y % DITHER_PERIOD) as usize];
    (DITHER_STRIDE * x + shift) % DITHER_PERIOD < level
}

/// 一块 1 像素周期光栅。
///
/// 它比抖动块**更早**暴露非 1.0 的缩放：抖动块糊掉要重采样核铺开好几个像素，
/// 而 1 像素周期的结构上任何一次非整数重采样都会掉幅度——原样贴上时是细密纹理，
/// 缩过一次就是一片平灰。
struct Grating {
    rect: Rect,
    kind: Ruling,
}

/// 光栅的四种走向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ruling {
    /// 棋盘：横竖两个方向同时 1 像素周期。
    Checker,
    /// 竖线：只在横向上有周期，暴露横向的缩放。
    Vertical,
    /// 横线：只在纵向上有周期，暴露纵向的缩放。
    Horizontal,
    /// 斜线：1 像素宽，周期取 3。
    ///
    /// 周期取 2 的斜线就是棋盘，与头一块重复；取 3 才既是 1 像素宽的线、又不与棋盘同形。
    Diagonal,
}

impl Grating {
    /// 一排四块，横排在 `band` 这一条带里，四种走向各占一块。
    fn row(band: Rect, gap: u32) -> Vec<Self> {
        let kinds = [
            Ruling::Checker,
            Ruling::Vertical,
            Ruling::Horizontal,
            Ruling::Diagonal,
        ];
        let count = kinds.len() as u32;
        spread(band, count, gap, spread_width(band, count, gap))
            .into_iter()
            .zip(kinds)
            .map(|(rect, kind)| Self { rect, kind })
            .collect()
    }

    fn draw(&self, canvas: &mut Canvas, hairline: u32) {
        let kind = self.kind;
        canvas.paint(
            self.rect,
            |x, y| if ruled(kind, x, y) { INK } else { PAPER },
        );
        canvas.outline(self.rect, hairline);
    }
}

/// 光栅上 `(x, y)` 那一格是不是墨黑。
fn ruled(kind: Ruling, x: u32, y: u32) -> bool {
    match kind {
        Ruling::Checker => (x + y).is_multiple_of(2),
        Ruling::Vertical => x.is_multiple_of(2),
        Ruling::Horizontal => y.is_multiple_of(2),
        Ruling::Diagonal => (x + y).is_multiple_of(3),
    }
}

/// 一条阶梯：一档候选位深在图上占的那一栏，自上而下由黑到白排满它的全部格点。
///
/// 并排是要求（14 号票）：各档位深的阶梯挨在一起，眼睛才比得出「这一条还分得开、
/// 下一条已经糊成一片」。分成几张图看，比的就成了记忆。
///
/// 一条阶梯上的一格叫**一级**，不叫一档：`CONTEXT.md` 里「档」已经归位深与阈值档位
/// （「对齐的那一档」「基准档」），而这里数的是灰度级，与「灰阶数」「级数」同一个词根。
struct Ladder {
    depth: BitDepth,
    rect: Rect,
}

impl Ladder {
    /// 第 `index` 级占的那一块，宽度与整条阶梯相同。
    ///
    /// 边界按比例算，而不是「级高 × 序号」累加：高度除不尽级数时，余数摊在整条阶梯上，
    /// 而不是全堆在最后一级。
    fn step(&self, index: u32) -> Rect {
        let levels = u64::from(self.depth.levels());
        let height = u64::from(self.rect.size.height);
        let start = (u64::from(index) * height / levels) as u32;
        let end = (u64::from(index + 1) * height / levels) as u32;
        Rect::new(
            self.rect.left,
            self.rect.top + start,
            self.rect.size.width,
            end - start,
        )
    }

    /// 级号的字号：放得进一级、又不越过这一栏的左半边，就印；放不下就整条都不印。
    ///
    /// 右半边留白是给眼睛的：号是帮着数的，不该把这一级的灰盖掉一大片。
    /// 放不下时整条阶梯都不印号——半数带号半数不带，数起来比全不带还难；
    /// 那时这一条仍数得出来，靠的是各级的灰本身两两不同。
    fn number_scale(&self, hairline: u32) -> Option<u32> {
        let widest = text_width(&self.depth.levels().to_string(), 1);
        let by_height = self.rect.size.height / self.depth.levels() / LINE_ADVANCE;
        let by_width = (self.rect.size.width / 2).saturating_sub(number_inset(hairline)) / widest;
        let scale = by_height.min(by_width).min(MAX_SCALE);
        (scale >= 1).then_some(scale)
    }

    /// 画这一条：抬头、逐级填灰并印上级号，最后沿**外沿**围一圈黑边。
    ///
    /// 黑边不是装饰：最亮的那一级就是纸白，不围起来它与图的底色连成一片，最后一级就数不出来了。
    /// 围在外沿而不是压在栏上，理由与那两排方块同一条（见 [`Canvas::outline`]）：
    /// 压上去就吃掉了最上与最下那一级——256 级那一条上一级只有两三行高，压一圈边就等于少两级。
    fn draw(&self, canvas: &mut Canvas, layout: &Layout) {
        self.draw_header(canvas, layout.header_scale);

        let number_scale = self.number_scale(layout.hairline);
        for index in 0..self.depth.levels() {
            let step = self.step(index);
            let level = grid_level(index, self.depth);
            canvas.fill(step, level);
            if let Some(scale) = number_scale {
                canvas.text(
                    step.left + number_inset(layout.hairline),
                    step.top + (step.size.height - GLYPH_HEIGHT * scale) / 2,
                    scale,
                    // 号与它压着的那一级必须分得开：暗的印白号，亮的印黑号。
                    if level >= 128 { INK } else { PAPER },
                    &(index + 1).to_string(),
                );
            }
        }
        canvas.outline(self.rect, layout.hairline);
    }

    /// 抬头居中排在这一栏正上方：这是哪一档位深、它有几级。
    fn draw_header(&self, canvas: &mut Canvas, scale: u32) {
        let header = header(self.depth);
        // 字号是按栏宽定的（见 [`fitting_scale`]），抬头因此排得下。`saturating_sub` 兜的是
        // 那个函数的下限：栏窄到连 1 号字都放不下时，宁可让抬头出栏，也不让它变成一次恐慌。
        let left = self.rect.left
            + self
                .rect
                .size
                .width
                .saturating_sub(text_width(&header, scale))
                / 2;
        canvas.text(
            left,
            self.rect.top - line_height(scale),
            scale,
            INK,
            &header,
        );
    }
}

/// 级号离栏左沿多远。留出边框，再空一点，号才不贴着框。
fn number_inset(hairline: u32) -> u32 {
    hairline * 3
}

/// 一条阶梯的抬头：这是哪一档位深、它有几级。
fn header(depth: BitDepth) -> String {
    format!("{depth} {}", depth.levels())
}

/// 印在图上的判读说明，按它挨着的那一段分组。
///
/// 中英两份都印（14 号票要英文，标定图批 01 号票加中文）。这是一个全中文界面的工具，
/// 「脱离文档也能用」对只有英文的说明并不成立；而英文那一份留着，
/// 是因为图会被拷到不认这套字的地方去看。
struct Legend {
    /// 这是哪台设备、哪块面板的图。
    heading: Vec<String>,
    /// 怎么打开，以及两件事的先后。
    order: Vec<String>,
    /// 英文那一份。
    english: Vec<String>,
    /// 第一节：像素完整性。紧挨着下面那两排方块。
    pixels: Vec<String>,
    /// 第二节：感知可分辨级数。紧挨着下面那几条阶梯。
    levels: Vec<String>,
}

impl Legend {
    /// 排在那两排方块**上面**的几组。
    fn above(&self) -> [&[String]; 4] {
        [&self.heading, &self.order, &self.english, &self.pixels]
    }

    /// 排在方块**下面**的那一组：第二节的说明，紧挨着几条阶梯。
    fn below(&self) -> &[String] {
        &self.levels
    }

    /// 每一行字，不论它排在哪一组。
    fn lines(&self) -> impl Iterator<Item = &String> {
        self.above().into_iter().flatten().chain(self.below())
    }

    /// 说明该用多大字号：宽度放得下，且给两排方块与阶梯留得出地方。
    ///
    /// 两头都要卡。只按宽度定的话，窄面板上说明会把阶梯挤没；只按高度定的话，
    /// 最长的那一行会出血。**阶梯那一块给的是下限而不是实得**——说明短的时候阶梯就更高，
    /// 那是好事，反过来则不行：阶梯矮到数不出级，这张图的第二件事就白做了。
    ///
    fn scale(&self, content: u32, available: u32, patch: u32) -> u32 {
        let widest = self
            .lines()
            .map(|line| text_width(line, 1))
            .max()
            .unwrap_or(1)
            .max(1);
        let by_width = content / widest;
        // 组与组、组与方块之间各空半行：上面四组之后各一个，两排方块之后各一个，
        // 下面那一组之后一个——七个半行。抬头另占一整行。
        let gaps = self.above().len() as u32 + 3;
        let rows = self.lines().count() as u32 + gaps.div_ceil(2) + 1;
        let spare = available
            // 阶梯至少要占五分之一：4bit 那一条 16 级，再少就一级不到 20 像素、数不出来。
            .saturating_sub(patch * 2 + available / 5)
            .max(rows * LINE_ADVANCE);
        let by_height = spare / (rows * LINE_ADVANCE);
        by_width.min(by_height).clamp(1, MAX_SCALE)
    }
}

/// 排出这块面板的判读说明。
///
/// 说明要点名**数哪一条**：几条阶梯就有几个数，不说清楚，回填给 `--gray-levels`
/// 的会是随便哪一个。数的是最细的那一条——阶梯按候选位深由小到大排开，它恒在最右边。
fn legend(profile: &Profile) -> Legend {
    let panel = profile.panel();
    let mut levels = vec![
        "二 感知可分辨级数".to_owned(),
        "数最右那条阶梯还分得开几级".to_owned(),
    ];
    // 只剩一条阶梯时不提「其余几条」：`--gray-levels 2` 就是这个样子。
    if BitDepth::candidates(panel.gray_levels).len() > 1 {
        levels.push("其余几条更粗，只作对照".to_owned());
    }
    levels.push("把那个数回填给 --gray-levels".to_owned());
    levels.push("数的是看得见的级数，不是物理灰阶数".to_owned());

    Legend {
        heading: vec![
            format!("TONEFIT 标定图  {}", profile.device()),
            format!(
                "{}X{}  {}PPI  {} 级",
                panel.resolution.width, panel.resolution.height, panel.ppi, panel.gray_levels
            ),
        ],
        order: vec![
            "以原尺寸打开，关掉缩放与裁边".to_owned(),
            "先做一，一不过就别做二".to_owned(),
            "阶梯被重采样过，数出的不是面板能显示的级数".to_owned(),
        ],
        // 英文那一份要把**三样检查各点一次**：只提抖动块的话，光栅与四角标记在英文里就不存在了，
        // 而 14 号票要的是「脱离文档也能用」——对着英文读的人拿不到另一份说明。
        english: vec![
            "SHOW AT 1:1. NO ZOOM, NO FIT, NO CROP.".to_owned(),
            "1 DITHER VS SOLID MUST DIFFER, GRATINGS MUST".to_owned(),
            "SHOW TEXTURE, ALL 4 CORNERS MUST BE THERE.".to_owned(),
            "2 IF SO, COUNT THE RIGHTMOST LADDER, PASS IT".to_owned(),
            "BACK AS --GRAY-LEVELS N. PERCEIVED, NOT SPEC.".to_owned(),
        ],
        pixels: vec![
            "一 像素完整性".to_owned(),
            "抖动块与实心块分得开吗，光栅有细纹吗".to_owned(),
            "四角标记都在吗，糊成一片就是被重采样过".to_owned(),
        ],
        levels,
    }
}

/// 图上的一块矩形：左上角，加上它的尺寸。
///
/// 版式算出来的每一块都是它——整条阶梯、一级、一块要在图上找的区域。
/// 四个数捆成一个类型，是因为它们处处一起走：拆成四个参数，调用处迟早会把顺序排错。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rect {
    left: u32,
    top: u32,
    size: Size,
}

impl Rect {
    const fn new(left: u32, top: u32, width: u32, height: u32) -> Self {
        Self {
            left,
            top,
            size: Size::new(width, height),
        }
    }
}

/// 一块按 8 位灰度作画的画布。
struct Canvas {
    size: Size,
    pixels: Vec<u8>,
}

impl Canvas {
    fn new(size: Size, fill: u8) -> Self {
        let pixels = vec![fill; (size.width as usize) * (size.height as usize)];
        Self { size, pixels }
    }

    /// 填一块矩形。越出画布的部分裁掉——版式算出来的块恒在画布内，这里只是不让它变成恐慌。
    fn fill(&mut self, rect: Rect, value: u8) {
        let right = (rect.left + rect.size.width).min(self.size.width);
        let bottom = (rect.top + rect.size.height).min(self.size.height);
        if rect.left >= right {
            return;
        }
        for row in rect.top..bottom {
            let start = row as usize * self.size.width as usize;
            self.pixels[start + rect.left as usize..start + right as usize].fill(value);
        }
    }

    /// 逐格填一块矩形，取值由 `value` 按**块内**坐标给出。
    ///
    /// 块内坐标而不是画布坐标：抖动块与光栅的图案必须跟着块走，
    /// 跟着画布走的话，同一档灰在图上换个位置就换了个花样。
    fn paint(&mut self, rect: Rect, value: impl Fn(u32, u32) -> u8) {
        let right = (rect.left + rect.size.width).min(self.size.width);
        let bottom = (rect.top + rect.size.height).min(self.size.height);
        for row in rect.top..bottom {
            for column in rect.left..right {
                let index = row as usize * self.size.width as usize + column as usize;
                self.pixels[index] = value(column - rect.left, row - rect.top);
            }
        }
    }

    /// 沿矩形内沿围一圈粗 `thickness` 的边。
    fn frame(&mut self, rect: Rect, thickness: u32, value: u8) {
        let Rect { left, top, size } = rect;
        self.fill(Rect::new(left, top, size.width, thickness), value);
        self.fill(
            Rect::new(left, top + size.height - thickness, size.width, thickness),
            value,
        );
        self.fill(Rect::new(left, top, thickness, size.height), value);
        self.fill(
            Rect::new(left + size.width - thickness, top, thickness, size.height),
            value,
        );
    }

    /// 沿矩形**外沿**围一圈边，一个格子都不动块里的像素。
    ///
    /// 方块要围起来才与纸白的底色分得开——最亮的那几块不围就没有边界。
    /// 而围在里面就改掉了块里的像素：抖动块与实心块的均值随之不再相等，
    /// 这张图最要紧的那条性质就没了。
    fn outline(&mut self, rect: Rect, thickness: u32) {
        let left = rect.left.saturating_sub(thickness);
        let top = rect.top.saturating_sub(thickness);
        self.frame(
            Rect::new(
                left,
                top,
                rect.size.width + (rect.left - left) + thickness,
                rect.size.height + (rect.top - top) + thickness,
            ),
            thickness,
            INK,
        );
    }

    /// 从 `(x, y)` 起印一行字，`scale` 是一个字模格子放大成几像素。
    ///
    /// 半宽与全宽混排：字距按各个字形自己的宽度推进（见 `glyphs`）。
    fn text(&mut self, x: u32, y: u32, scale: u32, value: u8, text: &str) {
        let mut pen = x;
        for character in text.chars() {
            self.draw_glyph(pen, y, scale, value, character);
            pen += advance(character) * scale;
        }
    }

    fn draw_glyph(&mut self, x: u32, y: u32, scale: u32, value: u8, character: char) {
        let Some(glyph) = glyphs::glyph(character) else {
            // 字模里没有的字符画成一个空框：印不出来这件事要看得见，而不是静静少一个字。
            let box_ = Rect::new(x, y, advance(character) * scale, GLYPH_HEIGHT * scale);
            self.frame(box_, scale, value);
            return;
        };
        for row in 0..GLYPH_HEIGHT {
            for column in 0..glyph.width() {
                if glyph.lit(column, row) {
                    self.fill(
                        Rect::new(x + column * scale, y + row * scale, scale, scale),
                        value,
                    );
                }
            }
        }
    }

    fn into_image(self) -> GrayImage {
        GrayImage::new(self.size, self.pixels)
    }
}

/// 行距：行与行之间空两格。
const LINE_ADVANCE: u32 = GLYPH_HEIGHT + 2;

/// 一个字模格子最多放大成几像素。再大就挤不下几行，说明反而说不完。
const MAX_SCALE: u32 = 3;

/// 这个字符占几格。字模里没有的字符按它那一类的宽度算——ASCII 半宽，其余全宽。
fn advance(character: char) -> u32 {
    glyphs::glyph(character).map_or(
        if character.is_ascii() {
            HALF_WIDTH
        } else {
            FULL_WIDTH
        },
        |glyph| glyph.width(),
    )
}

/// 一行字印出来多宽。字模自带左右留白，因此没有额外的字距要加。
fn text_width(text: &str, scale: u32) -> u32 {
    text.chars().map(advance).sum::<u32>() * scale
}

/// 一行字占的高度，含行距。
fn line_height(scale: u32) -> u32 {
    LINE_ADVANCE * scale
}

/// 这一行字排在 `available` 像素里，一个格子最大放得成几像素。
fn fitting_scale(text: &str, available: u32) -> u32 {
    (available / text_width(text, 1).max(1)).clamp(1, MAX_SCALE)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::quantize::{BitDepth, grid_level};

    /// 内置表里挑出来的几台设备，最窄的与最宽的面板都在里面。
    const DEVICES: [&str; 4] = ["boox-palma", "boox-poke6", "kobo-libra-2", "kindle-scribe"];

    /// `image` 上 `(x, y)` 那一点的取值。
    fn at(image: &GrayImage, x: u32, y: u32) -> u8 {
        image.pixels()[(y * image.size().width + x) as usize]
    }

    /// 一块矩形里全部像素的和，以及它有几个像素。
    fn sum_of(image: &GrayImage, rect: Rect) -> (u64, u64) {
        let mut sum = 0;
        let mut count = 0;
        for y in rect.top..rect.top + rect.size.height {
            for x in rect.left..rect.left + rect.size.width {
                sum += u64::from(at(image, x, y));
                count += 1;
            }
        }
        (sum, count)
    }

    /// 图上有没有哪一列，自上而下**接连**走过 `steps` 这一串取值。
    ///
    /// 逐列把取值游程编码，再找这一串是不是其中一段。「接连」是关键：
    /// 两级之间夹了别的取值就不算，那说明这条阶梯上有一级被别的东西盖住了。
    fn has_a_column_running_through(chart: &GrayImage, steps: &[u8]) -> bool {
        (0..chart.size().width).any(|x| contains(&column_runs(chart, x), steps))
    }

    /// 第 `x` 列自上而下的游程取值，相邻重复的并成一个。
    fn column_runs(chart: &GrayImage, x: u32) -> Vec<u8> {
        let mut runs: Vec<u8> = Vec::new();
        for y in 0..chart.size().height {
            let level = at(chart, x, y);
            if runs.last() != Some(&level) {
                runs.push(level);
            }
        }
        runs
    }

    /// `region` 里找得到 `stamp` 这块图案吗。
    fn contains_stamp(chart: &GrayImage, stamp: &GrayImage, region: Rect) -> bool {
        let last_x = (region.left + region.size.width)
            .min(chart.size().width)
            .saturating_sub(stamp.size().width);
        let last_y = (region.top + region.size.height)
            .min(chart.size().height)
            .saturating_sub(stamp.size().height);
        (region.top..=last_y)
            .any(|y| (region.left..=last_x).any(|x| matches_at(chart, stamp, x, y)))
    }

    fn matches_at(chart: &GrayImage, stamp: &GrayImage, left: u32, top: u32) -> bool {
        (0..stamp.size().height).all(|y| {
            (0..stamp.size().width).all(|x| at(chart, left + x, top + y) == at(stamp, x, y))
        })
    }

    /// `haystack` 里有没有 `needle` 这一段。
    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    /// 一行字印在 `paper` 底色上的那块图案，四周不留白——拿它去图上比对。
    fn stamp(text: &str, scale: u32, ink: u8, paper: u8) -> GrayImage {
        let size = Size::new(text_width(text, scale), GLYPH_HEIGHT * scale);
        let mut canvas = Canvas::new(size, paper);
        canvas.text(0, 0, scale, ink, text);
        canvas.into_image()
    }

    /// 画一张标定图并从 PNG 字节解回来——测的是**写出去的那张图**，不是中间缓冲。
    fn decode(profile: &Profile) -> GrayImage {
        let bytes = chart_png(profile).expect("画标定图");
        let mut decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
        let header = decoder.read_header_info().expect("读 PNG 头").clone();
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().expect("读 PNG 信息");
        let mut pixels = vec![0; reader.output_buffer_size().expect("PNG 缓冲尺寸")];
        let info = reader.next_frame(&mut pixels).expect("读 PNG 像素");
        pixels.truncate(info.buffer_size());
        // EXPAND 把调色板摊成 RGB8；色板项恒是三个相等的分量，取一个即可。
        if header.color_type == png::ColorType::Indexed {
            pixels = pixels.as_chunks::<3>().0.iter().map(|p| p[0]).collect();
        }
        GrayImage::new(Size::new(header.width, header.height), pixels)
    }

    /// 这一档位深的阶梯该长什么样：它的全部格点，由暗到亮。
    fn steps_of(depth: BitDepth) -> Vec<u8> {
        (0..depth.levels()).map(|i| grid_level(i, depth)).collect()
    }

    /// 标定图要**能 1:1 显示**：尺寸必须逐像素等于目标面板的分辨率。
    #[test]
    fn the_chart_is_exactly_the_panel_resolution() {
        for device in DEVICES {
            let profile = Profile::resolve(device).expect("内置型号");

            let chart = decode(&profile);

            assert_eq!(
                chart.size(),
                profile.panel().resolution,
                "{device} 的标定图贴不住面板"
            );
        }
    }

    /// **抖动块与同均值实心块的均值严格相等**（标定图批 01 号票的命门）。
    ///
    /// 不相等的话，阅读器重采样过之后两块仍然分得开，判读会给出一个假的「通过」——
    /// 那比没有这张图更糟。断言比的是两块的**和**：两块像素数相同，和相等即均值相等，
    /// 而和是整数，比得起「严格」这两个字，不必绕道浮点。
    ///
    /// 顺带钉住两块**不是同一块**：抖动那一半必须真有两种取值，
    /// 不然「均值相等」这条用两块实心也能满足，而那是一张什么都量不出来的图。
    #[test]
    fn a_dithered_patch_and_its_solid_twin_have_exactly_the_same_mean() {
        for device in DEVICES {
            let profile = Profile::resolve(device).expect("内置型号");
            let chart = decode(&profile);

            for pair in &Layout::plan(&profile).pairs {
                let (dithered, cells) = sum_of(&chart, pair.dither);
                let (solid, twin) = sum_of(&chart, pair.solid);

                assert_eq!(cells, twin, "{device}：两半的像素数不一样");
                assert_eq!(
                    dithered, solid,
                    "{device}：第 {} 档两半的均值不相等",
                    pair.level
                );
                let values: BTreeSet<u8> = (pair.dither.top
                    ..pair.dither.top + pair.dither.size.height)
                    .flat_map(|y| {
                        (pair.dither.left..pair.dither.left + pair.dither.size.width)
                            .map(move |x| (x, y))
                    })
                    .map(|(x, y)| at(&chart, x, y))
                    .collect();
                assert_eq!(
                    values,
                    [INK, PAPER].into_iter().collect(),
                    "{device}：抖动那一半不是墨黑与纸白两种取值"
                );
            }
        }
    }

    /// 四角的直角标记压在**第 0 行列与末行列**上，用来发现边距与裁切。
    ///
    /// 四个角上的那一点必须是墨黑：阅读器加了边距、或者裁掉了白边，第一行第一列就不在屏上，
    /// 少一个角当场看得出来。两条臂各自也要在，只有一个角点的话，屏边上那一个像素没人分得清。
    ///
    /// 同时钉住它**不是一圈边框**：每条边的中点是纸白。围成一圈的话，
    /// 裁掉一点边仍然看得见一条线，标记就答不出问题了。
    #[test]
    fn the_corner_marks_sit_on_the_first_and_last_row_and_column() {
        for device in DEVICES {
            let profile = Profile::resolve(device).expect("内置型号");
            let chart = decode(&profile);
            let (width, height) = (chart.size().width, chart.size().height);
            let arm = Layout::plan(&profile).arm;

            for (x, y) in [
                (0, 0),
                (width - 1, 0),
                (0, height - 1),
                (width - 1, height - 1),
            ] {
                assert_eq!(
                    at(&chart, x, y),
                    INK,
                    "{device}：({x}, {y}) 这个角上没有标记"
                );
            }
            // 两条臂：沿着第 0 行/末行走 `arm`，沿着第 0 列/末列也走 `arm`。
            for offset in 0..arm {
                for (x, y) in [
                    (offset, 0),
                    (width - 1 - offset, 0),
                    (offset, height - 1),
                    (width - 1 - offset, height - 1),
                    (0, offset),
                    (0, height - 1 - offset),
                    (width - 1, offset),
                    (width - 1, height - 1 - offset),
                ] {
                    assert_eq!(at(&chart, x, y), INK, "{device}：({x}, {y}) 处的臂断了");
                }
            }
            for (x, y) in [
                (width / 2, 0),
                (width / 2, height - 1),
                (0, height / 2),
                (width - 1, height / 2),
            ] {
                assert_eq!(
                    at(&chart, x, y),
                    PAPER,
                    "{device}：({x}, {y}) 上有东西——四角标记连成了一圈边框"
                );
            }
        }
    }

    /// 1 像素周期光栅四种都在图上：每一种在自己该有周期的那个方向上一格一换，
    /// 而且四种**互不相同**。
    ///
    /// 光栅比抖动块更早暴露非 1.0 的缩放，因此两样都要有，不是二选一。
    /// 四种互不相同这一条要单钉：只问「横向纵向变不变」的话，斜线与棋盘答得一模一样，
    /// 而那时图上就只剩三种走向了。
    #[test]
    fn the_four_gratings_alternate_every_pixel_and_are_all_different() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let chart = decode(&profile);
        let gratings = Layout::plan(&profile).gratings;

        assert_eq!(gratings.len(), 4, "四种光栅少了几种");
        let mut painted: Vec<(Ruling, Vec<u8>)> = Vec::new();
        for grating in &gratings {
            let rect = grating.rect;
            let (x, y) = (rect.left, rect.top);
            let (across, down) = (
                at(&chart, x + 1, y) != at(&chart, x, y),
                at(&chart, x, y + 1) != at(&chart, x, y),
            );
            let alternates = match grating.kind {
                Ruling::Checker | Ruling::Diagonal => across && down,
                Ruling::Vertical => across && !down,
                Ruling::Horizontal => !across && down,
            };
            assert!(alternates, "{:?} 这块光栅的周期不对", grating.kind);
            // 取一小块图案作指纹：走向不同的两块，六格见方之内必然已经分道扬镳。
            let window = (0..6)
                .flat_map(|row| (0..6).map(move |column| (column, row)))
                .map(|(column, row)| at(&chart, x + column, y + row))
                .collect();
            painted.push((grating.kind, window));
        }
        for (index, (kind, window)) in painted.iter().enumerate() {
            for (other, other_window) in &painted[index + 1..] {
                assert_ne!(window, other_window, "{kind:?} 与 {other:?} 是同一种图案");
            }
        }
    }

    /// 每一档候选位深各占一条阶梯，级与级在图上分得开（14 号票）。
    ///
    /// 断言不认版式：整幅图逐列扫下去，只要有一列接连走过这一位深的全部格点，
    /// 这条阶梯就在图上、每一级就都露着。阶梯照编码器的格点排，因此这一串取值
    /// 正是这一档位深真会写出的那些。
    #[test]
    fn every_candidate_bit_depth_gets_a_ladder_that_shows_all_of_its_steps() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");

        let chart = decode(&profile);

        for depth in BitDepth::candidates(profile.panel().gray_levels) {
            assert!(
                has_a_column_running_through(&chart, &steps_of(depth)),
                "{depth} 那条阶梯没在图上，或有几级被盖住了"
            );
        }
    }

    /// 阶梯止于面板灰阶数那道硬上界（ADR 0003）：e-ink 上没有 8bit 那一条，
    /// 而把灰阶数覆盖到 256 就有。图排的是**这台设备真会用到的**那几档，不是位深全集。
    #[test]
    fn the_ladders_stop_where_the_panel_gray_levels_do() {
        let eink = Profile::resolve("kobo-libra-2").expect("内置型号");
        let full = eink.clone().with_gray_levels(256).expect("2 与 256 之间");

        assert!(
            !has_a_column_running_through(&decode(&eink), &steps_of(BitDepth::Eight)),
            "e-ink 上不该有 8bit 那一条阶梯"
        );
        assert!(
            has_a_column_running_through(&decode(&full), &steps_of(BitDepth::Eight)),
            "灰阶数覆盖到 256 之后 8bit 那一条阶梯该在"
        );
    }

    /// 标定图以**无损**方式写出，一个像素都不许改（14 号票）。
    ///
    /// 它是量具，不是被处理的页：判据、上包络、抖动一概不碰它，像素以 8 位工作精度画出来，
    /// 解回来必须还是同一批字节。图上量出的级数要是被编码器动过手，量的就不是面板了。
    #[test]
    fn the_chart_comes_back_out_of_the_png_pixel_for_pixel() {
        for device in DEVICES {
            let profile = Profile::resolve(device).expect("内置型号");

            let painted = chart(&profile);
            let written = decode(&profile);

            assert_eq!(written.size(), painted.size(), "{device}");
            assert_eq!(
                written.pixels(),
                painted.pixels(),
                "{device} 的标定图被改过"
            );
        }
    }

    /// 判读说明要把整套做法说完，脱离文档也能用（14 号票、标定图批 01 号票）。
    ///
    /// 缺一不可的几件事：这是哪台设备、哪块面板的图，怎么显示，**两件事的先后**，
    /// 第一件看什么，第二件数什么、往哪填，以及那个数**是感知可分辨级数、
    /// 不是面板的物理灰阶数**（ADR 0003 的《后果》）。
    #[test]
    fn the_printed_instructions_say_how_to_read_the_chart() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");

        let legend = legend(&profile);
        let text: Vec<&str> = legend.lines().map(String::as_str).collect();
        let text = text.join("\n");

        assert!(text.contains("kobo-libra-2"), "{text}");
        assert!(text.contains("1264X1680"), "{text}");
        assert!(text.contains("16 级"), "{text}");
        // 怎么显示：不 1:1 的话两件事都不成立。
        assert!(text.contains("以原尺寸打开"), "{text}");
        assert!(text.contains("1:1"), "{text}");
        // 先后，连同为什么有先后。
        assert!(text.contains("先做一"), "{text}");
        assert!(text.contains("重采样"), "{text}");
        // 第一件事看什么。
        assert!(text.contains("像素完整性"), "{text}");
        assert!(text.contains("抖动块与实心块"), "{text}");
        assert!(text.contains("光栅"), "{text}");
        assert!(text.contains("四角标记"), "{text}");
        // 第二件事数什么、往哪填、数出来的是什么。
        assert!(text.contains("感知可分辨级数"), "{text}");
        assert!(text.contains("--gray-levels"), "{text}");
        assert!(text.contains("不是物理灰阶数"), "{text}");
        // 数哪一条要点名：几条阶梯就有几个数，不说清楚，回填的会是随便哪一个。
        assert!(text.contains("最右"), "{text}");
        // 英文那一份要**自成一套**：三样检查各点一次，脱离中文也走得下来（14 号票）。
        assert!(text.contains("1:1"), "{text}");
        assert!(text.contains("DITHER VS SOLID MUST DIFFER"), "{text}");
        assert!(text.contains("GRATINGS"), "{text}");
        assert!(text.contains("CORNERS"), "{text}");
        assert!(text.contains("RIGHTMOST LADDER"), "{text}");
        assert!(text.contains("--GRAY-LEVELS"), "{text}");
        assert!(text.contains("PERCEIVED"), "{text}");
    }

    /// 只剩一条阶梯时，说明不提「其余几条」——那时它们不存在。
    #[test]
    fn a_chart_with_one_ladder_does_not_mention_the_others() {
        let profile = Profile::resolve("kobo-libra-2")
            .expect("内置型号")
            .with_gray_levels(2)
            .expect("2 与 256 之间");

        let legend = legend(&profile);
        let text: Vec<&str> = legend.lines().map(String::as_str).collect();
        let text = text.join("\n");

        assert_eq!(BitDepth::candidates(2).len(), 1, "灰阶数 2 只留得下 1bit");
        assert!(!text.contains("其余几条"), "{text}");
        assert!(text.contains("最右"), "{text}");
    }

    /// **中文判读说明真的画进了图里**，不是只存在于源码常量里（标定图批 01 号票）。
    ///
    /// 逐台设备把每一行含汉字的说明按版式给的落点、字号重新印一遍，再与图上那一块逐像素比。
    /// 对得上，就说明这一行没被裁掉、没被别的东西盖住，字号也是按这块面板算出来的那一个。
    ///
    /// 它与 [`every_character_the_chart_can_print_has_a_glyph`] 是**一对**，各堵一半：
    /// 对照图与图走的是同一个 [`Canvas::text`]，字模缺了的话两边都画空框、这一条照样绿，
    /// 而那一条会当场报出少了哪个字。
    #[test]
    fn the_chinese_instructions_are_painted_on_the_chart() {
        for device in DEVICES {
            let profile = Profile::resolve(device).expect("内置型号");
            let layout = Layout::plan(&profile);
            let chart = decode(&profile);

            let chinese: Vec<&TextLine> = layout
                .text
                .iter()
                .filter(|line| !line.text.is_ascii())
                .collect();
            assert!(chinese.len() >= 8, "{device}：图上没有几行中文");

            for line in chinese {
                let printed = stamp(&line.text, line.scale, INK, PAPER);
                assert!(
                    line.left + printed.size().width <= chart.size().width
                        && line.top + printed.size().height <= chart.size().height,
                    "{device}：「{}」排到图外去了",
                    line.text
                );
                assert!(
                    matches_at(&chart, &printed, line.left, line.top),
                    "{device}：「{}」没印在图上",
                    line.text
                );
            }
        }
    }

    /// 英文那一份也在图上（14 号票）：图会被拷到不认这套字的地方去看。
    #[test]
    fn the_english_instructions_are_painted_on_the_chart() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let layout = Layout::plan(&profile);
        let chart = decode(&profile);

        let english = legend(&profile).english;
        // 先数一遍：英文那一组空掉的话，下面那个循环一遍都不走，用例就成了句空话。
        assert!(english.len() >= 4, "英文说明只剩 {} 行", english.len());
        for wanted in &english {
            let line = layout
                .text
                .iter()
                .find(|line| &line.text == wanted)
                .expect("英文那几行都在版式里");
            assert!(
                matches_at(
                    &chart,
                    &stamp(&line.text, line.scale, INK, PAPER),
                    line.left,
                    line.top
                ),
                "「{wanted}」没印在图上"
            );
        }
    }

    /// 每一级都带着自己的号：第几级，那一级的色块里就印着几（14 号票：每档可分别辨认）。
    ///
    /// 拿「这一级的灰」作底色去比对：找得到，说明号落在对的那一级上，
    /// 而且与底色分得开——号印在纯黑那一级上必须是白的，反过来也一样。
    #[test]
    fn every_step_of_a_ladder_carries_its_own_number() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");
        let chart = decode(&profile);
        let depth = BitDepth::Four;
        let ladder = Layout::plan(&profile)
            .ladders
            .into_iter()
            .find(|ladder| ladder.depth == depth)
            .expect("4bit 那一条阶梯");

        for index in 0..depth.levels() {
            let step = ladder.step(index);
            let level = grid_level(index, depth);
            let number = (index + 1).to_string();
            // 字号是版式算出来的，用例不抄那一段：从小到大试一遍，有一个对得上就算数。
            let found = [INK, PAPER].into_iter().any(|ink| {
                (1..=MAX_SCALE)
                    .any(|scale| contains_stamp(&chart, &stamp(&number, scale, ink, level), step))
            });
            assert!(found, "4bit 第 {} 级上没印着号", index + 1);
        }
    }

    /// 图上印得出的每一个字符都得有字模：少一个，说明就静静缺一块，而图是要脱离文档用的。
    ///
    /// 型号名归一后只剩 `[a-z0-9-]`（见 `profile` 的 `canonical`），覆盖住这一集合
    /// 就覆盖了内置表的全部型号，也覆盖了将来新加的那些行。
    #[test]
    fn every_character_the_chart_can_print_has_a_glyph() {
        let mut printed: String = "abcdefghijklmnopqrstuvwxyz0123456789-".to_owned();
        for device in DEVICES {
            let profile = Profile::resolve(device).expect("内置型号");
            let full = profile
                .clone()
                .with_gray_levels(256)
                .expect("2 与 256 之间");
            for profile in [profile, full] {
                let layout = Layout::plan(&profile);
                for line in &layout.text {
                    printed.push_str(&line.text);
                }
                for ladder in &layout.ladders {
                    printed.push_str(&header(ladder.depth));
                    for index in 0..ladder.depth.levels() {
                        printed.push_str(&(index + 1).to_string());
                    }
                }
            }
        }

        for character in printed.chars() {
            assert!(
                glyphs::glyph(character).is_some(),
                "字模里没有「{character}」"
            );
        }
    }

    /// 抖动块的构造本身立得住：每 15 格恰好 `level` 格纸白，而 255 除得尽 15。
    ///
    /// 均值严格相等这条性质是从这里推出来的：一行走完整数个周期，均值就恰好是 `17 × level`。
    /// 这一条钉的是推理的前提，上面那条用例钉的是图上真的成立。
    ///
    /// **每一档都要过，不只是图上取的那四档。** 这一条钉住的其实是散布步长与周期互素：
    /// 步长取 3 时，图上那四档（都是 3 的倍数）照样恰好，而第 1 档会一格不亮或亮三格——
    /// 那时抖动块变成一片周期 5 的规则条纹，重采样也抹不平它，量具就不量了。
    #[test]
    fn each_period_of_the_dither_holds_exactly_its_level_of_paper() {
        assert_eq!(
            u32::from(PAPER) % DITHER_PERIOD,
            0,
            "255 除不尽周期，均值就不是整数"
        );
        let mut shifts = ROW_SHIFT;
        shifts.sort_unstable();
        assert_eq!(
            shifts,
            std::array::from_fn::<u32, { DITHER_PERIOD as usize }, _>(|index| index as u32),
            "行间错位不是 0..15 的一个排列"
        );
        for level in 0..=DITHER_PERIOD {
            for y in 0..DITHER_PERIOD * 2 {
                for start in 0..DITHER_PERIOD {
                    let lit = (start..start + DITHER_PERIOD)
                        .filter(|&x| dithered(x, y, level))
                        .count() as u32;
                    assert_eq!(
                        lit, level,
                        "第 {level} 档在 y={y}、x={start} 处那一周期不对"
                    );
                }
            }
        }
    }

    /// 落盘在库里完成：父目录不在就建出来，写下的字节就是画出来的那张图（加固批 12 号票）。
    ///
    /// 命令行与会话共用的正是这个调用，钉在库这一侧因此两边都算钉住了，也不必起子进程。
    #[test]
    fn writing_the_chart_makes_its_parent_and_lays_down_the_bytes_it_drew() {
        let workspace = tempfile::tempdir().expect("建临时目录");
        let out = workspace.path().join("还不存在的目录").join("标定图.png");
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");

        write_chart(&profile, &out).expect("写标定图");

        assert_eq!(
            std::fs::read(&out).expect("读回标定图"),
            chart_png(&profile).expect("画标定图"),
            "落盘的字节与画出来的不是同一份"
        );
    }

    /// 写不出去的时候回的是 `Err`，不是恐慌（加固批 12 号票：这条路在库这一侧可测）——
    /// 调用方接得住，才谈得上「写出失败不崩掉一整个会话」（会话批的 13 号票）。
    ///
    /// 拿一个文件当父目录——建不出来，两台系统上都不行。
    #[test]
    fn writing_the_chart_where_the_parent_cannot_be_made_comes_back_as_an_error() {
        let workspace = tempfile::tempdir().expect("建临时目录");
        let blocker = workspace.path().join("这是一个文件");
        std::fs::write(&blocker, b"").expect("建挡路的文件");
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");

        let failure =
            write_chart(&profile, &blocker.join("标定图.png")).expect_err("父目录建不出来");

        // 说得清 = 说得出是哪件事、卡在哪个路径上。少了后者，会话只能印一句「写失败了」。
        let said = format!("{failure:#}");
        assert!(said.contains("标定图"), "没说出是哪件事：{said}");
        assert!(said.contains("这是一个文件"), "没说出卡在哪儿：{said}");
    }
}
