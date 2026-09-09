"""《纸白对齐》的上限：钳掉 N 级会连带抹掉多少「最淡的网点」。

`04` 的第六个数，问法与其余五个不同——**「钳掉 N 级之后，最淡的网点还在不在」**。

纸白对齐做的是 `[纸白, 255] → 255`（`tone-alignment/01`）。落在这个区间里的**非纸白**
像素会一起被抹平，而那些正是画面上最淡的网点。钳的宽度就是《离格量》`u`，
上限那个数限的是「`u` 到多大还肯钳」。

**这一半算得出来**：逐页量「纸白之上、255 之下」那一段里有多少像素，
按候选上限 N 分档累计。真机只需要在算出来的边界那几页上确认「看不看得见」，
不必从零开始扫。

用法：

    python survey_clamp_cost.py <一个或多个归档/目录> [--per 8]
"""

import argparse
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from survey_flat import offgrid, pages, paper_white  # noqa: E402

LIMITS = (1, 2, 3, 4, 6, 8, 12)


def clamp_cost(image: np.ndarray, white: int) -> dict:
    """钳 `[white, 255] → 255` 会抹掉哪些像素。

    抹掉的是**开区间** `(white, 255)` 里的那些——它们既不是纸白、也不是纯白，
    是画面上最淡的那一层网点。返回它们占页的比例，以及最淡那一层落在哪。
    """
    band = (image > white) & (image < 255)
    share = float(band.mean())
    return {
        "被抹掉占页": share,
        "band 内最暗": int(image[band].min()) if band.any() else -1,
        "纸白占页": float((image == white).mean()),
        "纯白占页": float((image == 255).mean()),
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("sources", type=Path, nargs="+")
    ap.add_argument("--per", type=int, default=8)
    args = ap.parse_args()

    rows = []
    for source in args.sources:
        for name, image in pages(source, args.per):
            white = paper_white(image)
            if white < 200:
                continue
            u = offgrid(white, 4)
            rows.append({"作品": source.stem[:22], "页": name, "纸白": white, "离格量": u,
                         **clamp_cost(image, white)})

    if not rows:
        print("没有量得出纸白的页")
        return

    print(f"逐作品：抽样 {len(rows)} 页\n")
    print(f"{'作品':<24}{'页数':>5}{'纸白':>6}{'离格量':>7}{'钳掉的那一段占页':>18}{'段内最暗':>10}")
    for work in dict.fromkeys(r["作品"] for r in rows):
        got = [r for r in rows if r["作品"] == work]
        whites = sorted({r["纸白"] for r in got})
        share = np.median([r["被抹掉占页"] for r in got])
        darkest = [r["band 内最暗"] for r in got if r["band 内最暗"] >= 0]
        print(
            f"{work:<24}{len(got):>5}{whites[0] if len(whites) == 1 else -1:>6}"
            f"{got[0]['离格量']:>7}{share:>18.4%}"
            f"{min(darkest) if darkest else -1:>10}"
        )

    print("\n\n按上限 N 分档：N 越大，肯钳的页越多，被抹掉的也越多\n")
    print(f"{'上限 N':>7}{'会被对齐的页':>14}{'占抽样':>9}{'这些页上被抹掉的中位':>22}{'最坏一页':>10}")
    for n in LIMITS:
        hit = [r for r in rows if 0 < r["离格量"] <= n]
        if not hit:
            print(f"{n:>7}{0:>14}{0:>9.1%}{'—':>22}{'—':>10}")
            continue
        shares = [r["被抹掉占页"] for r in hit]
        print(
            f"{n:>7}{len(hit):>14}{len(hit) / len(rows):>9.1%}"
            f"{np.median(shares):>22.4%}{max(shares):>10.4%}"
        )

    print("\n读法：「被抹掉的那一段」是纸白与纯白之间那些像素——画面上最淡的一层网点。")
    print("占页比例接近零，说明纸白之上本来就没有内容，钳掉不丢东西；")
    print("比例明显大于零的那几页，才是真机要去确认「看不看得见」的那几页。")


if __name__ == "__main__":
    main()
