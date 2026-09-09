"""平坦调阶梯 → 颗粒地板：真机的翻转点落在哪一格，地板就定在哪一段。

阶梯上判据的两档读数是闭式的，实测逐位对得上：

- **2bit 不抖** 读数恒等于《离格量》`u`（整块偏 `u`，低通项读 `u`、颗粒为零）。
  实测 2.000 / 8.000 / 21.000 / 42.000 —— 与 ADR 0002 决定第 5 条的推导逐位一致。
- **2bit+FS** 读数 = 低通残留 ＋ `max(sqrt(u(s−u)) − F, 0)`。
  **ADR 那条推导把低通残留当成零，实测不是**：`u=2` 上它就有 3.43~3.47，
  已经超过不抖的 2.00。低通核只有 4×4，FS 在这个尺度上还没把误差摊平。

于是「平坦调上抖动输不输」这件事，判据的答案随地板 `F` 移动，而**真机的答案是一个
定点**。两者交在哪一格，地板就标定在哪一段——这正是票面要的
「窗口两端各由某一页某一档的目视结论钉住」。

本脚本印的是那张对照表：每一个候选地板比例下，判据在哪一格上翻向 FS。
真机判完，查这张表即可。

用法：

    python ladder_calibrates_floor.py <ladder 目录>
"""

import json
import sys
from pathlib import Path

import numpy as np

STEP = 85.0  # 2bit 的《格点间距》


def flip_point(readings: dict, ladder: list, ratio: float) -> int | None:
    """给定地板比例，判据在哪一个《离格量》上开始判「FS 更好」。

    换地板只改颗粒项那一段。今天的地板是 0.215686，读数里的颗粒项因此是
    `max(起伏 − 0.215686×85, 0)`；换成 `ratio` 就把这一段重算。
    """
    live_floor = 0.215_686_27 * STEP
    new_floor = ratio * STEP
    for u in sorted({g["离格量"] for g in ladder}):
        rows = [g for g in ladder if g["离格量"] == u]
        flipped = []
        for g in rows:
            fs = readings[g["页"]]["读数"]["2bit+FS"]
            swing = g["FS高频起伏(sqrt(u(s-u)))"]
            fs_rest = fs - max(swing - live_floor, 0.0)  # 低通残留那一半
            fs_new = fs_rest + max(swing - new_floor, 0.0)
            flipped.append(fs_new < readings[g["页"]]["读数"]["2bit"])
        if all(flipped):
            return u
    return None


def main(out: Path) -> int:
    ladder = json.load(open(out / "阶梯.json", encoding="utf-8"))["格"]
    readings = json.load(open(out / "读数.json", encoding="utf-8"))

    print("判据在阶梯上的读数（三种背景亮度取中位，判据对亮度是盲的）\n")
    print(f"{'离格量 u':>8}{'撒点':>8}{'起伏':>8}{'2bit不抖':>10}{'2bit+FS':>10}   判据说")
    for u in sorted({g["离格量"] for g in ladder}):
        rows = [g for g in ladder if g["离格量"] == u]
        off = np.median([readings[g["页"]]["读数"]["2bit"] for g in rows])
        fs = np.median([readings[g["页"]]["读数"]["2bit+FS"] for g in rows])
        print(
            f"{u:>8}{rows[0]['FS在这块上要撒的点']:>8.1%}"
            f"{rows[0]['FS高频起伏(sqrt(u(s-u)))']:>8.2f}{off:>10.3f}{fs:>10.3f}"
            f"   {'不抖更好' if off < fs else 'FS 更好'}"
        )

    print("\n\n真机的翻转点 → 颗粒地板比例\n")
    print("（真机在哪一格开始判「FS 不比不抖差」，地板就落在对应的那一段）\n")
    print(f"{'地板比例':>10}{'1bit 地板':>11}{'2bit 地板':>11}   判据翻向 FS 的那一格")
    grid = sorted(
        {round(x, 4) for x in np.arange(0.0, 0.5001, 0.005)}
        | {0.080, 0.128, 0.2157, 0.2353, 0.45, 0.475}
    )
    seen = {}
    for ratio in grid:
        seen.setdefault(flip_point(readings, ladder, ratio), []).append(ratio)
    for u, ratios in sorted(seen.items(), key=lambda kv: (kv[0] is None, -(kv[0] or 0))):
        where = f"u = {u}" if u else "整条阶梯都判不抖更好"
        marks = [
            name
            for value, name in [
                (0.128, "ADR 算的上界 0.128"),
                (0.2157, "今天的取值 0.2157"),
                (0.2353, "盲测夹的上界 0.2353"),
                (0.45, "可行域下沿 0.45"),
                (0.475, "可行域上沿 0.475"),
            ]
            if min(ratios) <= value <= max(ratios)
        ]
        print(
            f"  地板比例 {min(ratios):.3f} ~ {max(ratios):.3f}"
            f"（1bit 地板 {min(ratios) * 255:5.1f} ~ {max(ratios) * 255:5.1f}）"
            f"   → 判据翻向 FS 的那一格：{where}"
            + (f"        含 {'、'.join(marks)}" if marks else "")
        )

    print("\n\n反过来读——真机答什么，地板就落在哪一段：\n")
    for u, ratios in sorted(seen.items(), key=lambda kv: (kv[0] is None, kv[0] or 0)):
        where = f"u = {u} 那一格上判「FS 不比不抖差」" if u else "整条阶梯上都判「不抖更干净」"
        print(f"  真机在 {where:<34} → 地板 {min(ratios):.3f} ~ {max(ratios):.3f}")

    print()
    print("  **u ≤ 2 那两格判据永远翻不过来**：FS 在那里的低通残留是 2.86 / 3.47，")
    print("  已经分别高过不抖的 1.00 / 2.00，而地板只改颗粒项那一段、改不动低通残留。")
    print("  ADR 0002 决定第 5 条那条上界（`F < s·(sqrt(r(1−r)) − r)`）把低通残留当成零，")
    print("  **代进实测之后那个约束根本不存在**——不是取值定错，是它推的那件事不会发生。")
    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1])))
