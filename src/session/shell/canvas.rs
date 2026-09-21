//! **画布**：往终端库的缓冲里逐格写字——设计稿的 `Screen` 那几手（`put`、`line`、`box`）
//! 在 Rust 这一侧的样子。
//!
//! 新界面**逐格照设计稿**（ADR 0019 决定第 13 条），而设计稿是一格一格摆出来的：框的抬头嵌在
//! 上边框第二格起、右上角那一段与右角之间留一格、底边右端那一段同理。终端库自带的 `Block`
//! 摆抬头的位置与它差一格，与其在 widget 的参数里凑，不如照设计稿的手法直接写格子——
//! 屏上每一格从哪儿来，读代码的人对着设计稿一眼看得出。
//!
//! # 宽字符
//!
//! 一个汉字占两格：写它的时候第二格清成空格、**样子跟着第一格**（终端库自己是整格清掉；
//! 设计稿那一格记着样式，被劈开时露出来的那半格带着它——补全框的右框线落在卷列表一个汉字
//! 中间时那一格就露出来了）；**写到一个宽字符的第二格上时，把那个宽字符换成空格**——
//! 不然整行多出或少掉一格，与设计稿的 `_split` 同一条。
//! 读回来那一头（`super::design::visible`）按显示宽度跳过第二格，两边因此一格对一格。

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use super::super::look::{Look, Segment, width_of};
use super::super::viewport::Scrollbar;
use super::paint;

/// 滚动条的滑块（设计稿 `box` 的 `scroll` 那一笔）：与粗框线同一个字，靠颜色分——滑块是默认色。
const THUMB: &str = "┃";

/// 借来的一屏缓冲。
pub(super) struct Canvas<'a> {
    buffer: &'a mut Buffer,
}

/// 一个框的四条边与它上面摆的字（不叫 `Frame`：终端库的一帧也叫那个名字，整屏画法里两个一起在场）。
pub(super) struct Border<'a> {
    /// 粗框线（聚焦那一块）还是细框线。
    pub(super) thick: bool,
    /// 框线的样子。
    pub(super) look: Look,
    /// 上边框左起那一段。
    pub(super) title: &'a [Segment],
    /// 上边框右端那一段。
    pub(super) right: &'a [Segment],
    /// 底边左起那一段。
    pub(super) bottom_left: &'a [Segment],
    /// 底边右端那一段。
    pub(super) bottom_right: &'a [Segment],
}

impl<'a> Canvas<'a> {
    pub(super) fn new(buffer: &'a mut Buffer) -> Self {
        Self { buffer }
    }

    /// 屏有几列。
    pub(super) fn width(&self) -> u16 {
        self.buffer.area.width
    }

    /// 屏有几行。
    pub(super) fn height(&self) -> u16 {
        self.buffer.area.height
    }

    /// 写到第 `x` 格之前：劈开了一个宽字符的话，把它换成空格（模块文档《宽字符》）。
    fn split(&mut self, x: u16, y: u16) {
        if x == 0 || x >= self.width() {
            return;
        }
        let before = self.buffer[(x - 1, y)].symbol();
        if crate::wrap::width(before) == 2 {
            self.buffer[(x - 1, y)].set_symbol(" ");
        }
    }

    /// 从第 `x` 格起写一截字，超出屏的右端就停；回写到了哪一格。
    pub(super) fn put(&mut self, mut x: u16, y: u16, text: &str, look: Look) -> u16 {
        if y >= self.height() {
            return x;
        }
        let style = paint::look(look);
        for glyph in text.chars() {
            let cells = crate::wrap::width(&glyph.to_string());
            if cells == 0 {
                continue;
            }
            if x.saturating_add(cells) > self.width() {
                break;
            }
            self.split(x, y);
            if cells == 2 {
                self.split(x + 1, y);
            }
            let cell = &mut self.buffer[(x, y)];
            cell.reset();
            cell.set_symbol(&glyph.to_string());
            cell.set_style(style);
            if cells == 2 {
                let half = &mut self.buffer[(x + 1, y)];
                half.reset();
                half.set_style(style);
            }
            x += cells;
        }
        x
    }

    /// 几截字接着写；`max_width` 给了就截在那里（一个宽字符装不下就停在它前面）。回写了几格。
    pub(super) fn line(
        &mut self,
        x: u16,
        y: u16,
        segments: &[Segment],
        max_width: Option<u16>,
    ) -> u16 {
        let mut used: u16 = 0;
        for segment in segments {
            let text = match max_width {
                Some(max) => {
                    let Some(left) = max.checked_sub(used).filter(|left| *left > 0) else {
                        break;
                    };
                    clip(&segment.text, left)
                }
                None => segment.text.clone(),
            };
            let end = self.put(x + used, y, &text, segment.look);
            used = end - x;
        }
        used
    }

    /// **整屏压暗**（设计稿 `dimAll`）：掀覆盖层之前把底下每一格都加上压暗，那一张再画在上面。
    pub(super) fn dim_all(&mut self) {
        let area = self.buffer.area;
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                self.buffer[(x, y)].modifier.insert(Modifier::DIM);
            }
        }
    }

    /// 一个框右边那条框线上的**滚动条**：滑块画多长、画在哪一截，照设计稿 `box` 的
    /// `scroll` 那一笔算（`CONTEXT.md` 的《视口》）——一道守卫加两条式子：
    ///
    /// ```text
    /// 共几行 ≤ 露出几行，或者框内高 ≤ 1 → 一格都不画
    /// 滑块长 = round(框内高 × 露出几行 ÷ 共几行)，至少一格
    /// 滑块位 = round((框内高 − 滑块长) × 从第几行画起 ÷ (共几行 − 露出几行))
    /// ```
    ///
    /// 头一行那道守卫也是设计稿那一笔自己的（`if (total > view && track > 1)`）：
    /// 没有可滚的东西时不画（`CONTEXT.md` 的《视口》末一句）。
    ///
    /// **自己算，不交给终端库自带的那个 widget**：这一屏要**逐格**照设计稿（ADR 0019
    /// 决定第 13 条），而那个 widget 的位置取整与设计稿差在 `.5` 那一格上——
    /// 从前交给它，滑块的长度对得上、位置在半格上偏一行（停车场 Q780 记的就是它；
    /// 「整卷统一灰阶」80×24 那一屏正落在半格上）。式子只有这三行，摆在这里比隔着
    /// 一层参数去凑它的取整读得清。
    ///
    /// **画滚动条的地方只有这一处**。`CONTEXT.md` 的《视口》末一句仍写着「走终端库
    /// 自带的那个 widget」——**词汇表与实现对不上，记在停车场 Q850，没有顺手改**。
    pub(super) fn scrollbar(&mut self, area: Rect, bar: &Scrollbar) {
        if area.width < 2 || area.height < 3 || bar.rows <= bar.window {
            return;
        }
        let track = f64::from(area.height - 2);
        let (rows, window, at) = (bar.rows as f64, bar.window as f64, bar.at as f64);
        if track <= 1.0 {
            return;
        }
        let thumb = (track * window / rows).round().max(1.0);
        let pos = ((track - thumb) * at / (rows - window).max(1.0)).round();
        let x = area.x + area.width - 1;
        let first = pos.clamp(0.0, track - thumb) as u16;
        for row in 0..(thumb as u16) {
            self.put(x, area.y + 1 + first + row, THUMB, Look::PLAIN);
        }
    }

    /// 一格格填上同一个字。
    pub(super) fn fill(&mut self, area: Rect, glyph: &str, look: Look) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                self.put(x, y, glyph, look);
            }
        }
    }

    /// 画一个框（设计稿的 `box`）：抬头嵌在上边框，右上角一段，底边左右各一段。
    /// 框比四格宽、两行高才画得出来。
    pub(super) fn frame(&mut self, area: Rect, border: &Border<'_>) {
        let Rect {
            x,
            y,
            width,
            height,
        } = area;
        if width < 4 || height < 2 {
            return;
        }
        let (tl, tr, bl, br, hz, vt) = if border.thick {
            ("┏", "┓", "┗", "┛", "━", "┃")
        } else {
            ("┌", "┐", "└", "┘", "─", "│")
        };
        let look = border.look;
        self.fill(
            area.inner(ratatui::layout::Margin::new(1, 1)),
            " ",
            Look::PLAIN,
        );
        let across = hz.repeat(usize::from(width - 2));
        self.put(x, y, &format!("{tl}{across}{tr}"), look);
        for row in y + 1..y + height - 1 {
            self.put(x, row, vt, look);
            self.put(x + width - 1, row, vt, look);
        }
        self.put(x, y + height - 1, &format!("{bl}{across}{br}"), look);
        let spaced = |segments: &[Segment]| -> Vec<Segment> {
            let mut out = vec![Segment::new(" ", look)];
            out.extend_from_slice(segments);
            out.push(Segment::new(" ", look));
            out
        };
        let mut right_width: u16 = 0;
        if !border.right.is_empty() {
            let segments = spaced(border.right);
            right_width = width_of(&segments) as u16;
            self.line(x + width - 2 - right_width, y, &segments, None);
        }
        if !border.title.is_empty() {
            let room = width.saturating_sub(4 + right_width);
            self.line(x + 1, y, &spaced(border.title), Some(room));
        }
        let mut bottom_right_width: u16 = 0;
        if !border.bottom_right.is_empty() {
            let segments = spaced(border.bottom_right);
            bottom_right_width = width_of(&segments) as u16;
            self.line(
                x + width - 2 - bottom_right_width,
                y + height - 1,
                &segments,
                None,
            );
        }
        if !border.bottom_left.is_empty() {
            let room = width.saturating_sub(5 + bottom_right_width);
            self.line(
                x + 1,
                y + height - 1,
                &spaced(border.bottom_left),
                Some(room),
            );
        }
    }
}

/// 截到 `width` 格：一个宽字符装不下就停在它前面（设计稿的 `clip`）。
pub(super) fn clip(text: &str, width: u16) -> String {
    let mut out = String::new();
    let mut used: u16 = 0;
    for glyph in text.chars() {
        let cells = crate::wrap::width(&glyph.to_string());
        if used + cells > width {
            break;
        }
        out.push(glyph);
        used += cells;
    }
    out
}

/// 补到 `width` 格宽（左对齐）；超了先截。
pub(super) fn padded(text: &str, width: u16) -> String {
    let text = clip(text, width);
    let gap = width.saturating_sub(crate::wrap::width(&text));
    format!("{text}{}", " ".repeat(usize::from(gap)))
}

/// 一个键在屏上的提示：`[键 → 做什么]`——方括号与那一句次要那一灰，键是默认色（设计稿的 `hint`）。
pub(super) fn hint(keys: &str, what: &str) -> Vec<Segment> {
    vec![
        Segment::faint("["),
        Segment::plain(keys),
        Segment::faint(format!(" → {what}]")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    fn canvas(width: u16, height: u16) -> Buffer {
        Buffer::empty(Rect::new(0, 0, width, height))
    }

    /// 一行读回来，宽字符占住的第二格跳过去（与探针 `visible` 同一条读法）。
    fn row(buffer: &Buffer, y: u16) -> String {
        let mut out = String::new();
        let mut x = 0;
        while x < buffer.area.width {
            let symbol = buffer[(x, y)].symbol();
            out.push_str(symbol);
            x += crate::wrap::width(symbol).max(1);
        }
        out
    }

    /// 写到一个宽字符的第二格上，那个宽字符换成空格，整行格数不变。
    #[test]
    fn writing_into_the_second_half_of_a_wide_glyph_splits_it() {
        let mut buffer = canvas(6, 1);
        {
            let mut canvas = Canvas::new(&mut buffer);
            canvas.put(0, 0, "设备", Look::PLAIN);
            canvas.put(1, 0, "x", Look::PLAIN);
        }
        assert_eq!(buffer[(0, 0)].symbol(), " ");
        assert_eq!(buffer[(1, 0)].symbol(), "x");
        assert_eq!(buffer[(2, 0)].symbol(), "备");
        assert_eq!(row(&buffer, 0).trim_end(), " x备");
        // 劈开露出来的那半格带着那个字的样子（设计稿 `_split`）。
        let mut buffer = canvas(6, 1);
        {
            let mut canvas = Canvas::new(&mut buffer);
            canvas.put(0, 0, "设备", Look::FAINT.dim());
            canvas.put(2, 0, "|", Look::PLAIN);
        }
        assert_eq!(buffer[(3, 0)].symbol(), " ");
        assert_eq!(
            buffer[(3, 0)].modifier,
            Modifier::DIM,
            "露出来的半格带着样子"
        );
        assert_eq!(buffer[(2, 0)].modifier, Modifier::empty());
    }

    /// 超出屏的右端就停：装不下的宽字符不写半个；`line` 截在给定的宽度上。
    #[test]
    fn text_stops_at_the_right_edge_and_at_the_given_width() {
        let mut buffer = canvas(5, 1);
        let mut canvas = Canvas::new(&mut buffer);
        let end = canvas.put(0, 0, "ab设备", Look::PLAIN);
        assert_eq!(end, 4, "「备」装不下");
        let used = canvas.line(
            0,
            0,
            &[Segment::plain("xy"), Segment::plain("设备")],
            Some(3),
        );
        assert_eq!(used, 2, "三格里装得下 xy，装不下「设」");
        assert_eq!(clip("设备x", 3), "设");
        assert_eq!(padded("设", 4), "设  ");
    }

    /// 框：抬头从第二格起、右上角那一段与右角之间留一格、底边右端同理；样子照要的画。
    #[test]
    fn a_frame_places_its_titles_like_the_design() {
        let mut buffer = canvas(14, 3);
        Canvas::new(&mut buffer).frame(
            Rect::new(0, 0, 14, 3),
            &Border {
                thick: true,
                look: Look::kind(crate::session::look::Kind::Focus),
                title: &[Segment::plain("任务")],
                right: &[Segment::plain("R")],
                bottom_left: &[],
                bottom_right: &[Segment::plain("2 of 5")],
            },
        );
        assert_eq!(row(&buffer, 0), "┏ 任务 ━━ R ━┓");
        assert_eq!(row(&buffer, 1), "┃            ┃");
        assert_eq!(row(&buffer, 2), "┗━━━ 2 of 5 ━┛");
        // 框线上的是聚焦那一色，抬头是默认色：都拿颜色一处（`paint::look`）的答案比，不自己点颜色名。
        let focus = paint::look(Look::kind(crate::session::look::Kind::Focus));
        assert_eq!(Some(buffer[(0, 0)].fg), focus.fg);
        assert_eq!(buffer[(2, 0)].fg, Color::Reset);
    }

    /// 整屏压暗给每一格都加上压暗、别的不动；之后再写的格从头算。
    #[test]
    fn dimming_the_whole_screen_adds_dim_to_every_cell_and_a_rewrite_starts_clean() {
        let mut buffer = canvas(4, 2);
        {
            let mut canvas = Canvas::new(&mut buffer);
            canvas.put(0, 0, "ab", Look::PLAIN.bold());
            canvas.dim_all();
            canvas.put(0, 1, "c", Look::PLAIN);
        }
        assert_eq!(buffer[(0, 0)].modifier, Modifier::BOLD | Modifier::DIM);
        assert_eq!(buffer[(3, 1)].modifier, Modifier::DIM, "没写过的格也压暗");
        assert_eq!(
            buffer[(0, 1)].modifier,
            Modifier::empty(),
            "压暗之后再写的格从头算"
        );
    }

    /// 滚动条：滑块的长度是 `round(track × view ÷ total)`、画在右框线上、默认色；
    /// 45 行露 20 行、从头画起时滑块占头 9 格，滚到底时贴着底。装得下时根本拿不到它（视口那一头）。
    #[test]
    fn the_scrollbar_thumb_is_sized_like_the_design_and_sits_on_the_right_border() {
        let area = Rect::new(0, 0, 10, 22);
        let thumb_rows = |at: usize| -> Vec<u16> {
            let mut buffer = canvas(10, 22);
            {
                let mut canvas = Canvas::new(&mut buffer);
                canvas.frame(
                    area,
                    &Border {
                        thick: true,
                        look: Look::kind(crate::session::look::Kind::Focus),
                        title: &[],
                        right: &[],
                        bottom_left: &[],
                        bottom_right: &[],
                    },
                );
                canvas.scrollbar(
                    area,
                    &Scrollbar {
                        rows: 45,
                        at,
                        window: 20,
                    },
                );
            }
            (1..21)
                .filter(|y| buffer[(9, *y)].fg == Color::Reset)
                .collect()
        };
        assert_eq!(thumb_rows(0), (1..=9).collect::<Vec<u16>>());
        assert_eq!(thumb_rows(1), (1..=9).collect::<Vec<u16>>());
        assert_eq!(
            thumb_rows(25),
            (12..=20).collect::<Vec<u16>>(),
            "滚到底贴着底"
        );
    }

    /// 样子照 `paint::look` 译：颜色、加粗、压暗各落在格上；每一格先清再写，不沾上一次的样子。
    #[test]
    fn a_look_lands_on_the_cell_and_a_rewrite_starts_clean() {
        let mut buffer = canvas(3, 1);
        {
            let mut canvas = Canvas::new(&mut buffer);
            canvas.put(
                0,
                0,
                "a",
                Look::tone(crate::session::tone::Tone::Caution).bold(),
            );
            canvas.put(0, 0, "b", Look::FAINT.dim());
        }
        let cell = &buffer[(0, 0)];
        assert_eq!(cell.symbol(), "b");
        assert_eq!(Some(cell.fg), paint::look(Look::FAINT).fg);
        assert_eq!(cell.modifier, Modifier::DIM, "上一次的加粗没沾上");
        assert_eq!(cell.bg, Color::Reset, "不设背景色");
    }
}
