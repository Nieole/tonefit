"""判据的读数出自**哪一块**？把那一块的性质与位置报出来。

`aggregate` 取的是「上分位与第 K 差之间更严的那个」秩上的**那一块**（`metric.rs::aggregate`）
——整页的读数就是它。于是有一个很直接的问题：**那一块长什么样？**

闸① 那些页上既有连续灰调，也有平坦纸底、对话框、线稿。判读者说的是**整页观感**
（「除了轻微色带之外几乎一致」），而判据只在读一块。**两者看的若不是同一处地方，
那矛盾就不在两项的标度上，在聚合上。**

报四样：那一块的**参照亮度**（是纸底还是中灰）、**活动度**（有没有结构）、
**颗粒**与**低通**，外加它在页面上的坐标——坐标可以拿去裁图，直接看那一块。

用法：

    python which_tile_decides.py <tiles.npz> [组前缀]
"""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from feasible import Tiles  # noqa: E402
from metric_replica import (  # noqa: E402
    LIVE,
    TAIL_TILES,
    TILE,
    UPPER_QUANTILE,
    masking_weight,
    quantisation_step,
)


def deciding_tile(tiles: Tiles, page, label: str):
    """聚合取到的那一块，连同它的各项。"""
    parts = tiles.parts(page, label)
    step = quantisation_step(parts["depth"])
    weight = masking_weight(parts["activity"], LIVE["masking_floor"], LIVE["masking_knee"])
    low = np.maximum(parts["low_pass_error"] - LIVE["low_pass_ratio"] * step, 0.0)
    grain = np.maximum(
        parts["grain"] - parts["ref_grain"] - LIVE["grain_ratio"] * step, 0.0
    )
    total = weight * (low + grain)
    n = total.size
    rank = max(max(int(np.ceil(UPPER_QUANTILE * n)), 1), n + 1 - min(TAIL_TILES, n)) - 1
    i = int(np.argsort(total)[rank])
    return {
        "块": i,
        "共几块": n,
        "读数": float(total[i]),
        "亮度": float(parts["tone"][i]),
        "活动度": float(parts["activity"][i]),
        "颗粒项": float(grain[i]),
        "低通项": float(low[i]),
        "该页亮度中位": float(np.median(parts["tone"])),
        "该页活动度中位": float(np.median(parts["activity"])),
    }


def main() -> int:
    tiles = Tiles(Path(sys.argv[1]))
    prefix = sys.argv[2] if len(sys.argv) > 2 else ""
    pages = sorted({(k.split("|")[0], k.split("|")[1]) for k in tiles.z if "|" in k})
    pages = [p for p in pages if p[0].startswith(prefix)]

    print(f"聚合取到的那一块（`2bit+FS`，今天的参数）。每块 {TILE}×{TILE}。\n")
    print(
        f"{'页':<30}{'块亮度':>7}{'页亮度中位':>10}{'块活动度':>8}{'页活动中位':>10}"
        f"{'颗粒项':>8}{'低通项':>8}"
    )
    rows = []
    for page in pages:
        try:
            d = deciding_tile(tiles, page, "2bit+FS")
        except KeyError:
            continue
        rows.append((page, d))
        print(
            f"{page[1][:28]:<30}{d['亮度']:>7.0f}{d['该页亮度中位']:>10.0f}"
            f"{d['活动度']:>8.1f}{d['该页活动度中位']:>10.1f}"
            f"{d['颗粒项']:>8.2f}{d['低通项']:>8.2f}"
        )

    if not rows:
        return 0
    print("\n读法：**块亮度**告诉你判据盯的是纸底还是中灰；**块活动度**比页中位低很多，")
    print("说明它挑的是那一页上最平的地方——而不是判读者在看的连续灰调。")
    bright = sum(1 for _, d in rows if d["亮度"] >= 230)
    quiet = sum(1 for _, d in rows if d["活动度"] < d["该页活动度中位"])
    print(f"\n  {len(rows)} 页里：块亮度 ≥230（近纸白）的有 **{bright}** 页；")
    print(f"  块活动度低于该页中位的有 **{quiet}** 页。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
