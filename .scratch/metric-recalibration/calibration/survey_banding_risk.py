"""普查每页的**色带风险面积**——「4bit 就近取整之后，这一页有多少地方会出现看得见的台阶」。

与 `survey_continuous_tone.continuous_tone_share` 的区别只在第二条判据，而那一条改变结论：

| | 第二条判据 | 量的是 |
|---|---|---|
| `continuous_tone_share` | 块内灰度极差 ≥ 12 | **平滑区面积**（一个代理量，阈值 12 是拍的） |
| 本文件 `banding_risk_share` | 与四邻块均值之差 ≥ 一个 4bit 格点（17） | **色带风险**（构造性的，没有拟合参数） |

第二条的构造性理由：**就近取整的色带，跨边对比恒等于一个格点间距**——相邻两块的参照均值
若差得过一个格点，量化后它们必然落到不同格点上，那条边就是一条台阶；差不过，就落同一格，没有边。
而误差扩散不产生阶跃边（它把误差摊到邻域里），所以这个量在 FS 上恒低。
顺带：整页同一灰度、只动直流平移出来的合成材料（L 组那种）按构造恒为 0——
不必再靠 `TONE_RANGE` 的上界去挡近白那几格。

## 它答了什么

「素材门分不分得开两侧」——把真机判过的五组页各算一遍：

| 组 | 真机结论 | n | 原定义 | 本文件 |
|---|---|---|---|---|
| 闸① 连续灰调 | 12/12 偏 `2bit+FS` | 12 | 31.48~63.41% | **33.99~52.20%** |
| A 组 线稿 | 8/8 分不出 | 8 | 9.03~22.40% | 8.71~18.78% |
| B 组 武器白底 | 4/4 偏 `4bit 不抖` | 4 | 6.35~29.86% | 6.22~21.27% |
| C 组 离格 0 | 6 平 · 2 偏 4bit | 8 | 11.43~29.24% | 10.01~24.83% |
| M 组 离格 1 | 4/4 偏 `4bit 不抖` | 4 | 15.49~31.82% | 12.03~22.97% |

**原定义分不开**：M 组 016 是 31.82%，压过闸① 下界 31.48% **0.33 个点**。
**本文件分得开**：闸① 下界 33.99% 对其余四组上界 24.83%，**间隔 9.16 点，36 页零重叠**。

> 这两列都是**只读参照 8bit 页**算出来的，不跑 tonefit、不重渲。素材不入库，所以
> 这一份记的是**怎么再算一遍**，数以本文件跑出来的为准。

用法：

    python survey_banding_risk.py <参照页目录或 cbz> [...]
"""

import io
import sys
import zipfile
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from metric_replica import TILE  # noqa: E402
from survey_continuous_tone import SMOOTH_MAX, TONE_RANGE  # noqa: E402
from survey_flat import local_variance  # noqa: E402

# 4bit 的《格点间距》。相邻两块均值差得过它，量化后必然落到不同格点上——那条边就是台阶。
STEP_4BIT = 255.0 / 15.0


def banding_risk_share(image: np.ndarray) -> float:
    """这一页有多少面积会在 4bit 就近取整下出现看得见的台阶。"""
    a = image.astype(np.float64)
    var = local_variance(image)
    height, width = a.shape
    rows, cols = (height - TILE) // TILE + 1, (width - TILE) // TILE + 1
    if rows < 3 or cols < 3:
        return 0.0

    means = np.zeros((rows, cols))
    smooth = np.zeros((rows, cols), dtype=bool)
    for i in range(rows):
        for j in range(cols):
            y, x = i * TILE, j * TILE
            block = a[y : y + TILE, x : x + TILE]
            means[i, j] = block.mean()
            smooth[i, j] = (
                float(np.median(var[y : y + TILE, x : x + TILE])) <= SMOOTH_MAX
                and TONE_RANGE[0] <= means[i, j] <= TONE_RANGE[1]
            )

    good = 0
    for i in range(1, rows - 1):
        for j in range(1, cols - 1):
            if not smooth[i, j]:
                continue
            step = max(
                abs(means[i, j] - means[i + di, j + dj])
                for di, dj in ((1, 0), (-1, 0), (0, 1), (0, -1))
            )
            if step >= STEP_4BIT:
                good += 1
    return good / (rows * cols)


def _pages(target: Path):
    if target.is_dir():
        for f in sorted(target.rglob("*")):
            if f.suffix.lower() in (".png", ".jpg", ".jpeg"):
                yield f.name, Image.open(f)
    else:
        with zipfile.ZipFile(target) as z:
            for n in sorted(x for x in z.namelist() if x.lower().endswith((".png", ".jpg", ".jpeg"))):
                yield Path(n).name, Image.open(io.BytesIO(z.read(n)))


def main(targets: list) -> int:
    for target in targets:
        path = Path(target)
        shares = []
        print(f"=== {path.name} ===")
        for name, image in _pages(path):
            share = banding_risk_share(np.asarray(image.convert("L")))
            shares.append(share)
            print(f"  {share * 100:6.2f}%  {name}")
        if shares:
            print(f"  -> {min(shares) * 100:.2f}% ~ {max(shares) * 100:.2f}%  (n={len(shares)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
