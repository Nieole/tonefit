use std::path::Path;

use anyhow::{Context, Result};

use crate::encode;
use crate::geometry::Size;
use crate::gray::GrayImage;
use crate::profile::Profile;
use crate::quantize::{BitDepth, grid_level};

/// 纸白。图的底色，也是阶梯最亮的那一档。
const PAPER: u8 = 255;

/// 墨黑。判读说明与边框的颜色，也是阶梯最暗的那一档。
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

/// 按目标 profile 画一张灰阶阶梯标定图，尺寸恒等于面板分辨率。
fn chart(profile: &Profile) -> GrayImage {
    let layout = Layout::plan(profile);
    let mut canvas = Canvas::new(profile.panel().resolution, PAPER);

    let mut y = layout.margin;
    for line in &layout.legend {
        canvas.text(layout.margin, y, layout.legend_scale, INK, line);
        y += line_height(layout.legend_scale);
    }
    for ladder in &layout.ladders {
        ladder.draw(&mut canvas, &layout);
    }
    canvas.into_image()
}

/// 一张标定图的版式：说明用多大字号排在上面，各条阶梯落在下面的哪一块。
///
/// 尺寸一概按面板算、不写死像素：面板从 824 宽到 1860 宽，同一个常数在两头一个嫌挤一个嫌空。
/// 整份版式一次算完，画的时候只管照着填——版式与作画分开，两边才不会各算一遍而算得不一样。
struct Layout {
    /// 图四周的留白，也是阶梯之间的间隔。
    margin: u32,
    /// 边框与细线的粗细。300 PPI 上一像素的线细到看不见。
    hairline: u32,
    /// 印在图上的判读说明，一行一项。
    legend: Vec<String>,
    /// 说明的字号：一个字模格子放大成几像素。
    legend_scale: u32,
    /// 阶梯抬头的字号。它按**栏宽**定，不跟着说明走——抬头得在自己那一栏里放得下。
    header_scale: u32,
    /// 各条阶梯，从左到右。
    ladders: Vec<Ladder>,
}

impl Layout {
    fn plan(profile: &Profile) -> Self {
        let panel = profile.panel();
        let resolution = panel.resolution;
        let margin = (resolution.width / 24).max(8);
        let content = resolution.width - margin * 2;

        let legend = legend(profile);
        let longest = legend.iter().map(|line| line.chars().count() as u32).max();
        let legend_scale = fitting_scale(longest.unwrap_or(1), content);

        // 位深按面板灰阶数裁（ADR 0003）：图排的是**这台设备真会用到的**那几档，不是位深全集。
        let depths = BitDepth::candidates(panel.gray_levels);
        let columns = depths.len() as u32;
        let width = (content - margin * (columns - 1)) / columns;
        // 抬头共用一个字号：各栏字号不一，眼睛会把它读成「这一条更要紧」。
        let header_scale = depths
            .iter()
            .map(|&depth| fitting_scale(header(depth).chars().count() as u32, width))
            .min()
            .expect("候选位深至少有 1bit 那一档");

        // 自上而下：说明、空一行、抬头，剩下的全归阶梯。
        let top = margin
            + line_height(legend_scale) * (legend.len() as u32 + 1)
            + line_height(header_scale);
        let height = resolution.height - margin - top;
        let ladders = depths
            .iter()
            .enumerate()
            .map(|(column, &depth)| Ladder {
                depth,
                rect: Rect::new(
                    margin + column as u32 * (width + margin),
                    top,
                    width,
                    height,
                ),
            })
            .collect();

        Self {
            margin,
            hairline: (resolution.width / 400).max(1),
            legend,
            legend_scale,
            header_scale,
            ladders,
        }
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
        let digits = self.depth.levels().to_string().chars().count() as u32;
        let by_height = self.rect.size.height / self.depth.levels() / LINE_ADVANCE;
        let by_width = (self.rect.size.width / 2).saturating_sub(number_inset(hairline))
            / (digits * GLYPH_ADVANCE);
        let scale = by_height.min(by_width).min(MAX_SCALE);
        (scale >= 1).then_some(scale)
    }

    /// 画这一条：抬头、逐级填灰并印上级号，最后围一圈黑边。
    ///
    /// 黑边不是装饰：最亮的那一级就是纸白，不围起来它与图的底色连成一片，最后一级就数不出来了。
    /// 它排在最后画，因此盖住的是各级自己的边沿，而不是反过来把级号盖住。
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
        canvas.frame(self.rect, layout.hairline, INK);
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
                .saturating_sub(printed_width(&header, scale))
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

/// 印在图上的判读说明（14 号票：图内含英文判读说明，脱离文档也能用）。
///
/// 用大写 ASCII 说。图是一张灰度位图，印字就得有字模，而中文字模按 16×16 算，
/// 一句话就是几十个字形——手写不出来，也验不了（见 [`GLYPHS`]）。
/// 中文那一份在 `calibrate` 的帮助文本里。终端上跑完只印当下要做对的那一件事，
/// 不把这套说法再抄一遍——同一段话摆三处，改的时候就得记着改三处。
///
/// 说明要点名**数哪一条**：几条阶梯就有几个数，不说清楚，回填给 `--gray-levels`
/// 的会是随便哪一个。数的是最细的那一条——阶梯按候选位深由小到大排开，它恒在最右边。
fn legend(profile: &Profile) -> Vec<String> {
    let panel = profile.panel();
    let mut lines = vec![
        "TONEFIT CALIBRATION CHART".to_owned(),
        format!("DEVICE {}", profile.device().to_uppercase()),
        format!(
            "PANEL {}X{} {}PPI {} LEVELS",
            panel.resolution.width, panel.resolution.height, panel.ppi, panel.gray_levels
        ),
        String::new(),
        "1 SHOW AT 1:1. NO ZOOM OR FIT.".to_owned(),
        "2 COUNT THE STEPS YOU CAN TELL".to_owned(),
        "  APART IN THE RIGHTMOST LADDER.".to_owned(),
    ];
    // 只剩一条阶梯时不提「其余几条」：`--gray-levels 2` 就是这个样子。
    if BitDepth::candidates(panel.gray_levels).len() > 1 {
        lines.push("  THE OTHERS ARE COARSER. THEY".to_owned());
        lines.push("  ARE THERE FOR COMPARISON.".to_owned());
    }
    lines.extend([
        "3 RUN TONEFIT AGAIN WITH THAT".to_owned(),
        "  COUNT AS --GRAY-LEVELS N.".to_owned(),
        String::new(),
        "WHAT YOU COUNT IS PERCEIVED LEVELS.".to_owned(),
        "IT IS NOT THE PHYSICAL GRAY LEVEL".to_owned(),
        "COUNT OF THE PANEL.".to_owned(),
    ]);
    lines
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

    /// 从 `(x, y)` 起印一行字，`scale` 是一个字模格子放大成几像素。
    fn text(&mut self, x: u32, y: u32, scale: u32, value: u8, text: &str) {
        for (index, character) in text.chars().enumerate() {
            let pen = x + index as u32 * GLYPH_ADVANCE * scale;
            self.draw_glyph(pen, y, scale, value, character);
        }
    }

    fn draw_glyph(&mut self, x: u32, y: u32, scale: u32, value: u8, character: char) {
        let cell = |column: usize, row: usize| {
            Rect::new(
                x + column as u32 * scale,
                y + row as u32 * scale,
                scale,
                scale,
            )
        };
        let Some(rows) = glyph(character) else {
            // 字模里没有的字符画成一个空框：印不出来这件事要看得见，而不是静静少一个字。
            let box_ = Rect::new(x, y, GLYPH_WIDTH * scale, GLYPH_HEIGHT * scale);
            self.frame(box_, scale, value);
            return;
        };
        for (row, cells) in rows.iter().enumerate() {
            for (column, lit) in cells.chars().enumerate() {
                if lit == '#' {
                    self.fill(cell(column, row), value);
                }
            }
        }
    }

    fn into_image(self) -> GrayImage {
        GrayImage::new(self.size, self.pixels)
    }
}

/// 字模的格子数：5 宽、7 高。
const GLYPH_WIDTH: u32 = 5;
const GLYPH_HEIGHT: u32 = 7;

/// 字距：字形之间空一格。
const GLYPH_ADVANCE: u32 = GLYPH_WIDTH + 1;

/// 行距：行与行之间空两格。
const LINE_ADVANCE: u32 = GLYPH_HEIGHT + 2;

/// 一个字模格子最多放大成几像素。再大就挤不下几个字，说明反而说不完。
const MAX_SCALE: u32 = 5;

/// 一行字排下来推进多少，含末尾那一格字距。定的是**下一行字从哪儿起**。
fn text_width(text: &str, scale: u32) -> u32 {
    text.chars().count() as u32 * GLYPH_ADVANCE * scale
}

/// 一行字**印出来**多宽，不含末尾那一格字距。居中与比对量的是它。
fn printed_width(text: &str, scale: u32) -> u32 {
    text_width(text, scale).saturating_sub(scale)
}

/// 一行字占的高度，含行距。
fn line_height(scale: u32) -> u32 {
    LINE_ADVANCE * scale
}

/// 这么多个字排在 `available` 像素里，一个格子最大放得成几像素。
fn fitting_scale(characters: u32, available: u32) -> u32 {
    (available / (characters.max(1) * GLYPH_ADVANCE)).clamp(1, MAX_SCALE)
}

/// `character` 的字模。大小写不论——表里只收大写。
fn glyph(character: char) -> Option<&'static [&'static str; GLYPH_HEIGHT as usize]> {
    let key = character.to_ascii_uppercase();
    GLYPHS
        .iter()
        .find(|(listed, _)| *listed == key)
        .map(|(_, rows)| rows)
}

/// 5×7 点阵字模，一行一个字形，`#` 是点亮的那一格。
///
/// 只收大写 ASCII，标点只收图上真用得到的那几个。判读说明要印在图上（14 号票），
/// 而图是一张灰度位图——印字就得有字模，字模就得随程序走，不能指望目标机器上有哪个字体。
///
/// 字母全集加数字全集都在表里，不只收当下说明里用到的那些：型号名归一后是 `[a-z0-9-]`
/// （见 `profile` 的 `canonical`），新加一行设备就可能带进任何一个字母。
/// 标点则**只加要用的**：有一条用例把图上印得出的每一个字符都拿来查一遍表，
/// 少一个会当场报出来，因此不必先囤着。
// 一行一个字形，眯起眼就看得出它长什么样——字模是画出来的，不是写出来的。
// rustfmt 会把每一行拆成七行，那七行摞起来什么也不像，改一个字形就得在脑子里重新拼一遍。
#[rustfmt::skip]
const GLYPHS: &[(char, [&str; GLYPH_HEIGHT as usize])] = &[
    (' ', ["     ", "     ", "     ", "     ", "     ", "     ", "     "]),
    ('-', ["     ", "     ", "     ", "#####", "     ", "     ", "     "]),
    ('.', ["     ", "     ", "     ", "     ", "     ", " ##  ", " ##  "]),
    (':', ["     ", " ##  ", " ##  ", "     ", " ##  ", " ##  ", "     "]),
    ('0', [" ### ", "#   #", "#  ##", "# # #", "##  #", "#   #", " ### "]),
    ('1', ["  #  ", " ##  ", "  #  ", "  #  ", "  #  ", "  #  ", " ### "]),
    ('2', [" ### ", "#   #", "    #", "   # ", "  #  ", " #   ", "#####"]),
    ('3', ["#####", "   # ", "  #  ", "   # ", "    #", "#   #", " ### "]),
    ('4', ["   # ", "  ## ", " # # ", "#  # ", "#####", "   # ", "   # "]),
    ('5', ["#####", "#    ", "#### ", "    #", "    #", "#   #", " ### "]),
    ('6', ["  ## ", " #   ", "#    ", "#### ", "#   #", "#   #", " ### "]),
    ('7', ["#####", "#   #", "    #", "   # ", "  #  ", "  #  ", "  #  "]),
    ('8', [" ### ", "#   #", "#   #", " ### ", "#   #", "#   #", " ### "]),
    ('9', [" ### ", "#   #", "#   #", " ####", "    #", "   # ", " ##  "]),
    ('A', [" ### ", "#   #", "#   #", "#####", "#   #", "#   #", "#   #"]),
    ('B', ["#### ", "#   #", "#   #", "#### ", "#   #", "#   #", "#### "]),
    ('C', [" ### ", "#   #", "#    ", "#    ", "#    ", "#   #", " ### "]),
    ('D', ["###  ", "#  # ", "#   #", "#   #", "#   #", "#  # ", "###  "]),
    ('E', ["#####", "#    ", "#    ", "#### ", "#    ", "#    ", "#####"]),
    ('F', ["#####", "#    ", "#    ", "#### ", "#    ", "#    ", "#    "]),
    ('G', [" ### ", "#   #", "#    ", "#  ##", "#   #", "#   #", " ### "]),
    ('H', ["#   #", "#   #", "#   #", "#####", "#   #", "#   #", "#   #"]),
    ('I', [" ### ", "  #  ", "  #  ", "  #  ", "  #  ", "  #  ", " ### "]),
    ('J', ["  ###", "   # ", "   # ", "   # ", "   # ", "#  # ", " ##  "]),
    ('K', ["#   #", "#  # ", "# #  ", "##   ", "# #  ", "#  # ", "#   #"]),
    ('L', ["#    ", "#    ", "#    ", "#    ", "#    ", "#    ", "#####"]),
    ('M', ["#   #", "## ##", "# # #", "#   #", "#   #", "#   #", "#   #"]),
    ('N', ["#   #", "##  #", "# # #", "#  ##", "#   #", "#   #", "#   #"]),
    ('O', [" ### ", "#   #", "#   #", "#   #", "#   #", "#   #", " ### "]),
    ('P', ["#### ", "#   #", "#   #", "#### ", "#    ", "#    ", "#    "]),
    ('Q', [" ### ", "#   #", "#   #", "#   #", "# # #", "#  # ", " ## #"]),
    ('R', ["#### ", "#   #", "#   #", "#### ", "# #  ", "#  # ", "#   #"]),
    ('S', [" ####", "#    ", "#    ", " ### ", "    #", "    #", "#### "]),
    ('T', ["#####", "  #  ", "  #  ", "  #  ", "  #  ", "  #  ", "  #  "]),
    ('U', ["#   #", "#   #", "#   #", "#   #", "#   #", "#   #", " ### "]),
    ('V', ["#   #", "#   #", "#   #", "#   #", "#   #", " # # ", "  #  "]),
    ('W', ["#   #", "#   #", "#   #", "# # #", "# # #", "## ##", "#   #"]),
    ('X', ["#   #", "#   #", " # # ", "  #  ", " # # ", "#   #", "#   #"]),
    ('Y', ["#   #", "#   #", " # # ", "  #  ", "  #  ", "  #  ", "  #  "]),
    ('Z', ["#####", "    #", "   # ", "  #  ", " #   ", "#    ", "#####"]),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantize::{BitDepth, grid_level};

    /// `image` 上 `(x, y)` 那一点的取值。
    fn at(image: &GrayImage, x: u32, y: u32) -> u8 {
        image.pixels()[(y * image.size().width + x) as usize]
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
        let size = Size::new(printed_width(text, scale), GLYPH_HEIGHT * scale);
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
        for device in ["kobo-libra-2", "boox-palma", "kindle-scribe"] {
            let profile = Profile::resolve(device).expect("内置型号");

            let chart = decode(&profile);

            assert_eq!(
                chart.size(),
                profile.panel().resolution,
                "{device} 的标定图贴不住面板"
            );
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
        for device in ["kobo-libra-2", "boox-palma"] {
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

    /// 判读说明要把整套做法说完，脱离文档也能用（14 号票）。
    ///
    /// 缺一不可的几件事：这是哪台设备、哪块面板的图，怎么显示，数什么，数出来的数往哪填，
    /// 以及那个数**是感知可分辨级数、不是面板的物理灰阶数**（ADR 0003 的《后果》）。
    #[test]
    fn the_printed_instructions_say_how_to_read_the_chart() {
        let profile = Profile::resolve("kobo-libra-2").expect("内置型号");

        let text = legend(&profile).join("\n");

        assert!(text.contains("KOBO-LIBRA-2"), "{text}");
        assert!(text.contains("1264X1680"), "{text}");
        assert!(text.contains("16 LEVELS"), "{text}");
        assert!(text.contains("1:1"), "{text}");
        assert!(text.contains("COUNT"), "{text}");
        assert!(text.contains("--GRAY-LEVELS"), "{text}");
        assert!(text.contains("PERCEIVED"), "{text}");
        assert!(text.contains("NOT THE PHYSICAL GRAY LEVEL"), "{text}");
        // 数哪一条要点名：几条阶梯就有几个数，不说清楚，回填的会是随便哪一个。
        assert!(text.contains("RIGHTMOST LADDER"), "{text}");
    }

    /// 只剩一条阶梯时，说明不提「其余几条」——那时它们不存在。
    #[test]
    fn a_chart_with_one_ladder_does_not_mention_the_others() {
        let profile = Profile::resolve("kobo-libra-2")
            .expect("内置型号")
            .with_gray_levels(2)
            .expect("2 与 256 之间");

        let text = legend(&profile).join("\n");

        assert_eq!(BitDepth::candidates(2).len(), 1, "灰阶数 2 只留得下 1bit");
        assert!(!text.contains("THE OTHERS"), "{text}");
        assert!(text.contains("RIGHTMOST LADDER"), "{text}");
    }

    /// 说明真的印在图上了：图上找得到照同一字号印出来的头一行。
    ///
    /// 这一条钉的是「印上去了」——没被裁掉、没被阶梯盖住、字号是按这块面板算出来的那一个。
    #[test]
    fn the_instructions_are_painted_on_the_chart() {
        for device in ["kobo-libra-2", "boox-palma", "kindle-scribe"] {
            let profile = Profile::resolve(device).expect("内置型号");
            let layout = Layout::plan(&profile);
            let chart = decode(&profile);

            let title = stamp(&layout.legend[0], layout.legend_scale, INK, PAPER);
            let band = Rect::new(0, 0, chart.size().width, chart.size().height / 3);

            assert!(
                contains_stamp(&chart, &title, band),
                "{device}：判读说明没印在图上"
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
        for device in ["kobo-libra-2", "boox-palma", "kindle-scribe"] {
            let profile = Profile::resolve(device).expect("内置型号");
            let full = profile
                .clone()
                .with_gray_levels(256)
                .expect("2 与 256 之间");
            for profile in [profile, full] {
                let layout = Layout::plan(&profile);
                printed.push_str(&layout.legend.join(""));
                for ladder in &layout.ladders {
                    printed.push_str(&header(ladder.depth));
                    for index in 0..ladder.depth.levels() {
                        printed.push_str(&(index + 1).to_string());
                    }
                }
            }
        }

        for character in printed.chars() {
            assert!(glyph(character).is_some(), "字模里没有「{character}」");
        }
    }

    /// 字模表本身要立得住：每个字形恰好 5×7、只有点亮与不亮两种格子，
    /// 而且没有两个字形长得一模一样——印出来分不开的字等于没印。
    #[test]
    fn the_glyphs_are_five_by_seven_and_all_different() {
        for (character, rows) in GLYPHS {
            for row in rows {
                assert_eq!(
                    row.chars().count() as u32,
                    GLYPH_WIDTH,
                    "「{character}」有一行不是 {GLYPH_WIDTH} 格宽"
                );
                assert!(
                    row.chars().all(|cell| cell == '#' || cell == ' '),
                    "「{character}」有一格既不是 # 也不是空"
                );
            }
        }
        for (index, (character, rows)) in GLYPHS.iter().enumerate() {
            for (other, other_rows) in &GLYPHS[index + 1..] {
                assert_ne!(rows, other_rows, "「{character}」与「{other}」长得一样");
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
