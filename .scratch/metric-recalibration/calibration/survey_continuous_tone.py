"""普查每页的**连续灰调面积**，给闸① 补测分层选页用。

闸① 问的是「连续灰调上真机偏哪一档」，现有 n=4（AB 交错本第 9~12 对），
`03` 的落地记录自己写着「方向清楚但样本薄，**不作定量依据**」——而它恰恰是
现在卡住可行域的那一端。补测要把 n 提到 16。

**按连续灰调面积分层挑，不按判据读数挑。**按读数挑等于用判据给自己出题：
挑出来的会是「判据觉得有争议的页」，而那时结果说不了「真机在连续灰调上普遍偏哪一档」。

## 什么算「连续灰调」

逐块（32×32，与判据同一个块尺度）判三条，三条都满足才算：

- **平滑**：块内 3×3 邻域方差的中位数低——没有线稿、文字、网点那种高频
- **不是纯平坦**：块内灰度有起伏（极差够大）——纯白纸底与纯黑不算连续灰调
- **落在中间调**：块均值离纯白与纯黑都有距离

页的「连续灰调面积」＝这样的块占页面的比例。

用法：

    python survey_continuous_tone.py <参照8bit 目录> [--top 40]
"""

import argparse
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from metric_replica import TILE  # noqa: E402
from survey_flat import local_variance  # noqa: E402

SMOOTH_MAX = 12.0  # 块内 3×3 方差中位数的上限：再高就有线稿/网点了
RANGE_MIN = 12  # 块内灰度极差的下限：再低就是纯平坦，不是「调」
TONE_RANGE = (40, 235)  # 块均值要落在这中间——纯白纸底与纯黑不算


def continuous_tone_share(image: np.ndarray) -> float:
    """这一页有多少面积是连续灰调。"""
    var = local_variance(image)
    height, width = image.shape
    good = total = 0
    for y in range(0, height - TILE + 1, TILE):
        for x in range(0, width - TILE + 1, TILE):
            block = image[y : y + TILE, x : x + TILE]
            total += 1
            if float(np.median(var[y : y + TILE, x : x + TILE])) > SMOOTH_MAX:
                continue
            if int(block.max()) - int(block.min()) < RANGE_MIN:
                continue
            mean = float(block.mean())
            if not (TONE_RANGE[0] <= mean <= TONE_RANGE[1]):
                continue
            good += 1
    return good / total if total else 0.0


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("pages", type=Path)
    ap.add_argument("--top", type=int, default=40)
    args = ap.parse_args()

    rows = []
    for page in sorted(args.pages.rglob("*.png")):
        image = np.asarray(Image.open(page).convert("L"), dtype=np.uint8)
        rows.append((page.name, continuous_tone_share(image), image.shape))

    rows.sort(key=lambda r: -r[1])
    print(f"共 {len(rows)} 页。连续灰调面积最大的 {args.top} 页：\n")
    print(f"{'页':<44}{'连续灰调面积':>12}{'尺寸':>14}")
    for name, share, shape in rows[: args.top]:
        print(f"{name:<44}{share:>12.1%}{shape[1]}x{shape[0]:>9}")

    band = [0.0, 0.05, 0.10, 0.20, 0.30, 0.50, 1.01]
    print(f"\n全卷分布：")
    for lo, hi in zip(band, band[1:]):
        n = sum(1 for _, s, _ in rows if lo <= s < hi)
        print(f"  {lo:>5.0%} ~ {hi:>5.0%}   {n:>4} 页  {'█' * (n * 40 // len(rows))}")


if __name__ == "__main__":
    main()
