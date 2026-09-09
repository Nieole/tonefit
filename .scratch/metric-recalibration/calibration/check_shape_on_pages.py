"""拿 L 组试出来的形状去验真实页：B 组、C 组、闸① 上判定档对不对。

`try_luminance.py` 在 L 组十五格上试出一个形状（颗粒项乘一道按块亮度的权重），
**但 L 组是平坦块**——单一灰度、逐块亮度相同、而且不抖那一侧没有色带。
真实页上每块亮度各异，判定还要跨六个候选选一档。**形状必须在这里再过一遍。**

约束与 `feasible.py` 同一套（逐页的判定档，见那里的 `WANTED`）：
B 组不得低于 4bit、C 组该是 `2bit+FS`、闸① 落在真机说更干净的那一档。
每页解出一个阈值区间，全部求交——交集非空，这个形状就同时满足了真实页那一侧。

用法：

    python check_shape_on_pages.py <tiles.npz> [形状 a 颗粒地板比例]
"""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from feasible import ASCENDING, WANTED, Tiles, _aggregate  # noqa: E402
from metric_replica import masking_weight, quantisation_step  # noqa: E402
from try_luminance import luminance_weight  # noqa: E402


def page_reads(parts, grain_ratio, low_pass_ratio, m_floor, m_knee, shape, a, q=0.0) -> float:
    """一页一档的读数：**两项各按块的参照亮度加权**。

    颗粒项乘 `luminance_weight`（形状由 `shape`/`a` 定）；低通项乘 `(y/255)^q`
    ——`q > 0` 是暗处缩小、`q < 0` 是暗处放大、`q = 0` 退回今天。

    低通项那一侧必须也能加权：B 组与闸① 的 `2bit+FS` 读数**几乎全是低通项**
    （3.25 对 6.19），而真机说前者不可接受、后者可接受——**低通项大的反而可接受**。
    只给颗粒项加权动不了这一对。
    """
    step = quantisation_step(parts["depth"])
    weight = masking_weight(parts["activity"], m_floor, m_knee)
    tone = np.clip(parts["tone"] / 255.0, 0.02, 1.0)
    low = np.maximum(parts["low_pass_error"] - low_pass_ratio * step, 0.0) * tone**q
    grain = np.maximum(parts["grain"] - parts["ref_grain"] - grain_ratio * step, 0.0)
    return float(_aggregate(weight * (low + grain * luminance_weight(parts["tone"], shape, a))))


def window(tiles: Tiles, params, shape, a, q=0.0):
    """逐页解阈值区间再求交。返回 (下界, 上界, 卡住下界的页, 卡住上界的页, 逐页读数)。"""
    lo, hi = -np.inf, np.inf
    lo_page = hi_page = None
    table = []
    for page, wanted in WANTED:
        reads = {d: page_reads(tiles.parts(page, d), *params, shape, a, q) for d in ASCENDING}
        if wanted == "≥4bit":
            one_lo, one_hi = -np.inf, min(reads[d] for d in ASCENDING[:4])
        else:
            below = ASCENDING[: ASCENDING.index(wanted)]
            one_lo = reads[wanted]
            one_hi = min((reads[d] for d in below), default=np.inf)
        if one_lo > lo:
            lo, lo_page = one_lo, (page, wanted)
        if one_hi < hi:
            hi, hi_page = one_hi, (page, wanted)
        table.append((page, wanted, reads, one_lo, one_hi))
    return lo, hi, lo_page, hi_page, table


def main() -> int:
    npz = Path(sys.argv[1])
    shape = sys.argv[2] if len(sys.argv) > 2 else "step"
    a = float(sys.argv[3]) if len(sys.argv) > 3 else 0.75
    grain_ratio = float(sys.argv[4]) if len(sys.argv) > 4 else 0.020
    tiles = Tiles(npz)
    params = (grain_ratio, 0.0, 0.5, 8.0)

    print(f"形状：颗粒项 × {shape}(a={a:g}) · 颗粒地板比例 {grain_ratio:g}")
    print("（掩蔽仍取今天的 0.5 / 8.0，低通地板 0）\n")

    lo, hi, lo_page, hi_page, table = window(tiles, params, shape, a)
    print(f"{'页':<30}{'要判成':<9}{'1bit':>8}{'1b+FS':>8}{'2bit':>8}"
          f"{'2b+FS':>8}{'4bit':>8}")
    for page, wanted, reads, *_ in table:
        print(
            f"{page[1][:28]:<30}{wanted:<9}"
            + "".join(f"{reads[d]:>8.2f}" for d in ASCENDING[:5])
        )

    print(f"\n阈值窗口 [{lo:.3f}, {hi:.3f})  {'**非空**' if lo < hi else '**空**'}")
    print(f"  下界 ← {lo_page[0][1]} 要判成 {lo_page[1]}")
    print(f"  上界 ← {hi_page[0][1]} 要判成 {hi_page[1]}")

    if lo < hi:
        th = (lo + hi) / 2
        print(f"\n取阈值 {th:.3f}，逐页判定：\n")
        wrong = 0
        for page, wanted, reads, *_ in table:
            got = next((d for d in ASCENDING if reads[d] <= th), ASCENDING[-1])
            ok = (got in ("4bit", "4bit+FS")) if wanted == "≥4bit" else (got == wanted)
            wrong += not ok
            print(f"  {page[1][:28]:<30}判成 {got:<9}要 {wanted:<9}{'✓' if ok else '✗'}")
        print(f"\n{len(table) - wrong}/{len(table)} 页对。")
        return 0 if wrong == 0 else 1
    return 1


if __name__ == "__main__":
    sys.exit(main())


def joint_search(tiles, ladder, ladder_reads, shapes, grains, floors, knees, qs=(0.0,)):
    """两边同时搜：L 组十五格的成绩 ＋ 真实页那十六页的阈值窗口。

    **两边都要过才算数。**L 组是平坦块（比同一页两档的大小），真实页要跨六个候选
    选一档——同一个形状在一边成立、另一边未必。`04` 那一轮的教训就是只盯一边。
    """
    from try_luminance import score_shape

    best = []
    for shape, a in shapes:
        for f in floors:
            for k in knees:
                for g in grains:
                  for q in qs:
                    right, _ = score_shape(
                        tiles, ladder, ladder_reads, (g, 0.0, f, k), shape, a, -q
                    )
                    lo, hi, lo_page, hi_page, _ = window(tiles, (g, 0.0, f, k), shape, a, q)
                    best.append(
                        {
                            "形状": shape,
                            "a": a,
                            "q": q,
                            "地板": g,
                            "FLOOR": f,
                            "KNEE": k,
                            "L组": right,
                            "窗口": (lo, hi),
                            "宽": hi - lo,
                            "卡住的": (lo_page[0][1], hi_page[0][1]),
                        }
                    )
    return best
