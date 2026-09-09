"""试形状：给判据加一维**背景亮度**，有没有一种形状配得上真机。

`04` 的真机判读证明了：同一个《离格量》、不同背景亮度，人给出相反的答案，
而今天的判据对亮度是盲的。缺的是一整项——**但「加成什么形状」还没有答案**。

这一支是那个问题的试验台。**不跑真机、不改 Rust**：逐块四原始量已经导出（`tiles.npz`），
L 组十五格连同真机答案也在，于是任何候选形状都能立刻问一句「它答对几格」。

## 判分用的那套题

| 来源 | 题 | 标准答案 |
|---|---|---|
| L 组 15 格 | 平坦调上，`2bit+FS` 对 `2bit 不抖`，谁更干净 | 判读者答的（`平` 算两边都对） |
| B 组 4 页 | 判定档 | 不得低于 4bit |
| C 组 8 页 | 判定档 | `2bit+FS` |
| 闸① 4 页 | 判定档 | 3 页 `2bit+FS`、1 页 ≥4bit |

**L 组那 15 格是这套题里最要紧的**——三行分歧就在里面，而分歧正是今天的判据
无论怎么标定都配不上的那部分。

## 候选形状

亮度权重 `w(y)` 乘在**颗粒项**上（`y` 是那一块在**参照**上的均值，0~255）。
乘在颗粒项而不是整块读数上，是因为真机说的正是「同样的撒点，在亮底上看得见、
在暗底上看不见」——那是颗粒的可见度随亮度变，不是整幅误差随亮度变。

三种形状，都归一到「近白处 = 1」，各带一个可调参数：

- `weber`：`(y/255)^p` —— 韦伯定律的粗形式，亮处敏感、暗处迟钝
- `linear`：`1 − k·(1 − y/255)` —— 线性衰减，`k` 是暗端衰到多少
- `step`：分段——亮于 `cut` 的按 1、暗于它的按 `low`

用法：

    python try_luminance.py <tiles.npz> <ladder 目录> <真机包目录> "L: ..."         [<复判包目录> "R: ..."]

给了复判包就用它修正那几格：两次一致取那个答案，两次相反记成「平」。
"""

import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from decode_answers import decode  # noqa: E402
from feasible import ASCENDING, B_WHITE, C_ENOUGH, GATE_ONE, Tiles, _aggregate  # noqa: E402
from metric_replica import LIVE, masking_weight, quantisation_step  # noqa: E402

STEP2 = quantisation_step(2)


def luminance_weight(y: np.ndarray, shape: str, a: float) -> np.ndarray:
    """亮度权重，归一到近白处 ≈ 1。`y` 是块在参照上的均值。"""
    t = np.clip(y / 255.0, 0.0, 1.0)
    if shape == "weber":
        return t**a
    if shape == "linear":
        return np.maximum(1.0 - a * (1.0 - t), 0.0)
    if shape == "step":
        return np.where(t >= a, 1.0, 0.25)
    if shape == "none":
        return np.ones_like(t)
    raise ValueError(shape)


def page_score(parts, tone, grain_ratio, low_pass_ratio, m_floor, m_knee, shape, a) -> float:
    """一页一档的读数，颗粒项上多乘一道亮度权重。"""
    step = quantisation_step(parts["depth"])
    weight = masking_weight(parts["activity"], m_floor, m_knee)
    low = np.maximum(parts["low_pass_error"] - low_pass_ratio * step, 0.0)
    grain = np.maximum(parts["grain"] - parts["ref_grain"] - grain_ratio * step, 0.0)
    return float(_aggregate(weight * (low + grain * luminance_weight(tone, shape, a))))


def recheck_truth(recheck_bundle: Path, line: str) -> dict:
    """复判包的答案 → `{格: 修正后的真值}`。

    同一格放了两次、左右顺序相反：**两次一致**就是那一格的真值（上一轮若与它不同，
    上一轮那个是噪声）；**两次相反**说明它在判读边界上，记成 `平`——
    边界上的答案是随机的，不该拿去定形状。
    """
    rows = decode(recheck_bundle, [line])["R_复判"]
    picks = {}
    for r in rows:
        if r["这一格是"] == "复判":
            picks.setdefault(r["格"], []).append(r["判读者选的"])
    fixed = {}
    for cell, both in picks.items():
        fixed[cell] = both[0] if len(both) == 2 and both[0] == both[1] else "平"
    return fixed


def ladder_questions(ladder_dir: Path, bundle: Path, line: str, fixed: dict | None = None):
    """L 组十五格：每格给出（背景灰度、离格量、真机答案）。

    `fixed` 是复判修正过的那几格，覆盖头一轮的答案。
    """
    rows = decode(bundle, [line])["L_平坦调阶梯"]
    if fixed:
        for r in rows:
            if r["格"] in fixed:
                r["判读者选的"] = fixed[r["格"]]
    return [
        {
            "格": r["格"],
            "页": r["页"],
            "灰度": r["目标灰度"],
            "u": r["离格量"],
            "起伏": r["FS高频起伏"],
            "真机": {"2bit不抖": "不抖", "2bit+FS": "FS", "平": "平"}[r["判读者选的"]],
        }
        for r in rows
    ]


def low_pass_weight(y: np.ndarray, q: float) -> np.ndarray:
    """低通项那一侧的亮度权重，归一到近白处 = 1。

    **方向与颗粒项相反**：直流偏移的可见度按韦伯定律走 `ΔL / L`，
    同样的绝对偏移在暗底上相对更大、因此更显眼。取 `(y/255)^(−q)`，
    `q = 0` 退回今天（不随亮度变）。
    """
    t = np.clip(y / 255.0, 0.02, 1.0)
    return t ** (-q)


def score_shape(tiles, ladder, ladder_reads, params, shape, a, q=0.0):
    """这一组参数 ＋ 这个形状，答对几格。

    两项各乘各的亮度权重：颗粒项 `luminance_weight`（暗处衰减）、
    低通项 `low_pass_weight`（暗处放大）。
    """
    grain_ratio, low_pass_ratio, m_floor, m_knee = params
    live_floor = LIVE["grain_ratio"] * STEP2
    right = 0
    detail = []
    for cell in ladder:
        # 阶梯是单一平坦调：整块的参照亮度就是那个灰度，两档的读数都是闭式的
        y = np.array(float(cell["灰度"]))
        fs_read = ladder_reads[cell["页"]]["读数"]["2bit+FS"]
        residue = fs_read - max(cell["起伏"] - live_floor, 0.0)
        grain = max(cell["起伏"] - grain_ratio * STEP2, 0.0)
        wg = float(luminance_weight(y, shape, a))
        wl = float(low_pass_weight(y, q))
        fs = residue * wl + grain * wg
        off = float(cell["u"]) * wl  # 不抖在平坦调上是纯低通项，读数恒等于 u
        says = "不抖" if off < fs else "FS"
        ok = cell["真机"] == "平" or says == cell["真机"]
        right += ok
        detail.append((cell, says, ok))
    return right, detail


def main() -> int:
    npz, ladder_dir, bundle, line, *rest = sys.argv[1:]
    tiles = Tiles(Path(npz))
    fixed = recheck_truth(Path(rest[0]), rest[1]) if len(rest) >= 2 else None
    if fixed:
        print("复判修正：", "，".join(f"{k} → {v}" for k, v in fixed.items()), "\n")
    ladder = ladder_questions(Path(ladder_dir), Path(bundle), line, fixed)
    ladder_reads = json.load(open(Path(ladder_dir) / "读数.json", encoding="utf-8"))

    print("L 组十五格：每种形状答对几格（`平` 算两边都对，满分 15）\n")
    grid = {
        "none": [0.0],
        "weber": [0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0],
        "linear": [0.3, 0.5, 0.7, 0.85, 1.0],
        "step": [0.55, 0.65, 0.75, 0.85],
    }
    grain_grid = np.round(np.arange(0.0, 0.5001, 0.02), 4)

    q_grid = np.round(np.arange(0.0, 1.601, 0.1), 2)
    print("每次刷新纪录印一行——`q = 0` 那一列就是「只给颗粒项加亮度」的老结果。\n")
    print(f"{'颗粒项形状':<10}{'a':>6}{'低通 q':>8}{'成绩':>9}{'颗粒地板比例':>14}")
    best_overall = None
    for shape, params in grid.items():
        for a in params:
            for q in q_grid:
                best = max(
                    (
                        score_shape(
                            tiles, ladder, ladder_reads, (g, 0.0, 0.5, 8.0), shape, a, q
                        )[0],
                        g,
                    )
                    for g in grain_grid
                )
                if best_overall is None or best[0] > best_overall[0]:
                    best_overall = (best[0], shape, a, best[1], q)
                    print(f"{shape:<10}{a:>6.2f}{q:>8.1f}{best[0]:>7}/15{best[1]:>14.3f}  ←")

    right, shape, a, grain, q = best_overall
    print(
        f"\n\n最好的一组：颗粒项 {shape} a={a:g} · 低通项 q={q:g} ·"
        f" 颗粒地板比例 {grain:.3f} —— **{right}/15**\n"
    )
    _, detail = score_shape(tiles, ladder, ladder_reads, (grain, 0.0, 0.5, 8.0), shape, a, q)
    print(f"{'格':<12}{'灰度':>5}{'u':>4}{'颗粒权重':>9}{'低通权重':>9}"
          f"{'判据说':>8}{'真机说':>8}{'':>4}")
    for cell, says, ok in sorted(detail, key=lambda d: (d[0]["u"], -d[0]["灰度"])):
        y = np.array(float(cell["灰度"]))
        print(
            f"{cell['格']:<12}{cell['灰度']:>5}{cell['u']:>4}"
            f"{float(luminance_weight(y, shape, a)):>9.3f}"
            f"{float(low_pass_weight(y, q)):>9.3f}"
            f"{says:>8}{cell['真机']:>8}{'  ✓' if ok else '  ✗'}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
