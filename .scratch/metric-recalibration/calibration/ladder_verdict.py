"""L 组判完之后：颗粒地板能不能配上真机的答案。

阶梯上判据的两档读数是闭式的：

    2bit 不抖 = u                     （整块偏 u，低通项读 u、颗粒为零）
    2bit+FS   = 低通残留 + max(起伏 − F, 0)

**地板 `F` 只动第二项**，而低通残留与 `u` 都不随它变。于是「判据在这一格上说谁更好」
随 `F` 怎么移，是能逐格解出来的：

    判据要说「FS 更好」  ⟺  低通残留 + max(起伏 − F, 0) < u

左边随 `F` 单调不增，最小值是**低通残留本身**（`F` 大到吃掉整个颗粒项）。因此

    **低通残留 ≥ u 的那一格，任何 `F` 都翻不过来。**

再加一条判据的构造事实：**它对背景亮度是盲的**——同一个 `u` 的三格（近白/中灰/偏暗）
读数逐位几乎相同。所以真机若在同一个 `u` 上给出不同的答案，判据**无论怎么标定
都配不上**，缺的不是一个取值，是一整项。

用法：

    python ladder_verdict.py <ladder 目录> <真机包目录> "L: ..."
"""

import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from decode_answers import decode  # noqa: E402
from metric_replica import LIVE, quantisation_step  # noqa: E402

STEP = quantisation_step(2)
TONES = ["近白", "中灰", "偏暗"]


def main() -> int:
    ladder_dir, bundle, *lines = sys.argv[1:]
    rows = decode(Path(bundle), lines)["L_平坦调阶梯"]
    readings = json.load(open(Path(ladder_dir) / "读数.json", encoding="utf-8"))

    print("=== 判据在每一格上的两档读数，与真机答的 ===\n")
    print(f"{'u':>4}{'背景':>6}{'不抖=u':>8}{'FS 读数':>9}{'其中低通残留':>13}"
          f"{'判据说':>9}{'真机说':>11}{'':>4}")
    live_floor = LIVE["grain_ratio"] * STEP
    never = []
    for row in sorted(rows, key=lambda r: (r["离格量"], TONES.index(r["背景"]))):
        u = row["离格量"]
        swing = row["FS高频起伏"]
        fs = readings[row["页"]]["读数"]["2bit+FS"]
        residue = fs - max(swing - live_floor, 0.0)  # 低通残留：地板动不了的那一半
        metric = "不抖" if u < fs else "FS"
        truth = {"2bit不抖": "不抖", "2bit+FS": "FS", "平": "平"}[row["判读者选的"]]
        mark = "  ✓" if truth in ("平", metric) else "  ✗"
        if truth == "FS" and residue >= u:
            never.append((u, row["背景"], residue))
            mark = "  ✗ 任何地板都翻不过来"
        print(
            f"{u:>4}{row['背景']:>6}{float(u):>8.2f}{fs:>9.3f}{residue:>13.2f}"
            f"{metric:>9}{truth:>11}{mark}"
        )

    print("\n\n=== 一、真机在同一个 u 上给出不同答案的那几行 ===\n")
    print("判据对背景亮度是盲的——同一个 u 的三格读数逐位几乎相同（上表 `FS 读数` 一列可验）。")
    print("真机若在同一行里分歧，判据无论怎么标定都配不上。\n")
    split = 0
    for u in sorted({r["离格量"] for r in rows}):
        got = {r["背景"]: r["判读者选的"] for r in rows if r["离格量"] == u}
        picks = {v for v in got.values() if v != "平"}
        if len(picks) > 1:
            split += 1
            detail = " · ".join(f"{t}:{got[t]}" for t in TONES if t in got)
            print(f"  u = {u:<3} **分歧**  {detail}")
    print(f"\n  {split} 行分歧。")

    print("\n\n=== 二、任何地板都翻不过来的那几格 ===\n")
    print("FS 的低通残留高过不抖的读数 u，而地板只削颗粒项、削不动低通残留。\n")
    if never:
        for u, tone, residue in never:
            print(f"  u = {u} {tone}：真机说 FS 更干净，而 FS 的低通残留 {residue:.2f} ≥ u = {u}")
        print(f"\n  {len(never)} 格。**这几格单独就否掉了「调地板能配上真机」。**")
    else:
        print("  没有。")

    print("\n\n=== 三、那么地板还剩什么可标的 ===\n")
    print("撇开上面两类格子，剩下的格子要求地板落在哪：")
    lo, hi = 0.0, 1.0
    for row in sorted(rows, key=lambda r: r["离格量"]):
        u, swing = row["离格量"], row["FS高频起伏"]
        fs = readings[row["页"]]["读数"]["2bit+FS"]
        residue = fs - max(swing - live_floor, 0.0)
        truth = {"2bit不抖": "不抖", "2bit+FS": "FS", "平": "平"}[row["判读者选的"]]
        if truth == "平" or residue >= u:
            continue
        # 要判成 FS：residue + max(swing − F, 0) < u  →  F > swing − (u − residue)
        # 要判成不抖：反之
        edge = (swing - (u - residue)) / STEP
        if truth == "FS":
            lo = max(lo, edge)
            print(f"  u={u:<3}{row['背景']}  真机 FS  → 地板比例要 > {edge:.3f}")
        else:
            hi = min(hi, edge)
            print(f"  u={u:<3}{row['背景']}  真机 不抖 → 地板比例要 < {edge:.3f}")
    print(f"\n  合起来：地板比例要 > {lo:.3f} 且 < {hi:.3f}"
          f" —— {'非空' if lo < hi else '**空的**'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
