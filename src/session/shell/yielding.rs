//! 新界面**摆不下时谁让位**，次序只有这一处（`CONTEXT.md` 的《会话》：让位；spec《尺寸与让位》）。
//!
//! 宽度：卷列表砍列（随树那一票接进 `super::super::columns`）、配置视图不到 90 列单栏、
//! 确认条不到 110 列短句、横条先收窄后让掉；高度：屏底恒一行，不到 30 行总览正文两行，
//! 卷列表吃剩下的高度。**不到 60×16 整屏只剩窗口太小**（[`too_small`]）。
//! 本票落地的是最小尺寸、总览那一档与开跑之前路径那一列的宽度；其余各档随各票接进来。

use ratatui::layout::Rect;

/// 整屏画得出东西的最小尺寸：列 × 行。
pub(super) const LEAST: (u16, u16) = (60, 16);

/// 不到这么多行时总览正文收成两行。
const COMPACT_BELOW: u16 = 30;

/// 窗口太小：只剩「窗口太小」那几行（`CONTEXT.md` 的《让位》）。
pub(super) fn too_small(screen: Rect) -> bool {
    screen.width < LEAST.0 || screen.height < LEAST.1
}

/// 总览正文收不收成两行。
pub(super) fn compact(screen: Rect) -> bool {
    screen.height < COMPACT_BELOW
}

/// 开跑之前路径那一列有多宽：最长那条路径加两格，至少 20 格、至多让行尾那一句留 40 格
/// （设计稿 `drawRow` 的 `pathW`）。`inner` 是框里能写字的宽度。
pub(super) fn path_column(inner: u16, longest: u16) -> u16 {
    longest
        .saturating_add(2)
        .min(inner.saturating_sub(40))
        .max(20)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 60×16 起画得出整屏，差一列或一行都算窗口太小；不到 30 行总览收成两行。
    #[test]
    fn the_smallest_screen_is_sixty_by_sixteen_and_thirty_rows_compacts_the_overview() {
        assert!(!too_small(Rect::new(0, 0, 60, 16)));
        assert!(too_small(Rect::new(0, 0, 59, 16)));
        assert!(too_small(Rect::new(0, 0, 60, 15)));
        assert!(compact(Rect::new(0, 0, 80, 24)));
        assert!(!compact(Rect::new(0, 0, 120, 36)));
    }

    /// 路径那一列：最长的加两格，夹在 20 与「留 40 格给行尾」之间。
    #[test]
    fn the_path_column_follows_the_longest_path_within_its_bounds() {
        assert_eq!(path_column(116, 24), 26);
        assert_eq!(path_column(116, 8), 20);
        assert_eq!(path_column(76, 60), 36);
    }
}
