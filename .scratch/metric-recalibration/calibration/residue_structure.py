"""低通残留的**空间结构**：它是簇状的，还是均匀的？

走到这一步，矛盾已经收窄到一处：窗口的两端都是「FS 的低通残留」——

    B 组白底（纸白 253，撒点 2.4%）   残留 3.25   真机说**看得见**
    闸① 连续灰调（撒点约 50%）        残留 6.19   真机说**看不见**

**残留更大的那个反而看不见。**幅度解释不了，韦伯定律也解释不了（闸① 的相对对比度
反而更高）。剩下的解释是**空间结构**：FS 的误差扩散在占空比接近 0 或 1 时会结出
簇状伪影（worms/clumping），在 50% 附近最均匀——这是 FS 的已知特性。
簇是低频的、成片的，人眼看得见；均匀的细密噪声是高频的，看不见。

## 怎么量

把残留图**再按几个尺度低通一遍**，看 RMS 怎么衰减：

- **均匀**的残留：能量都在高频，核一放大就被抹掉，RMS 掉得快
- **簇状**的残留：能量在低频，放大核也抹不掉，RMS 掉得慢

「掉得快慢」＝ `RMS(核=32) / RMS(核=4)`，叫它**低频占比**。判据今天只读 `RMS(核=4)`
那一个数，读不出这个比。

用法：

    python residue_structure.py <参照页> <候选页> [<参照页> <候选页> ...]
"""

import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from metric_replica import low_pass  # noqa: E402

SCALES = (4, 8, 16, 32)


def residue(reference: np.ndarray, candidate: np.ndarray, kernel: int = 4) -> np.ndarray:
    """低通后的局部均值误差图——判据低通项量的就是它的 RMS。"""
    return low_pass(reference, kernel).astype(np.float64) - low_pass(candidate, kernel).astype(
        np.float64
    )


def main() -> int:
    args = sys.argv[1:]
    print("低通残留按尺度衰减（残留图再低通一遍，看 RMS 掉多少）\n")
    print(
        f"{'页':<26}" + "".join(f"{'核' + str(k):>9}" for k in SCALES) + f"{'低频占比':>10}"
    )
    rows = []
    for i in range(0, len(args), 2):
        ref = np.asarray(Image.open(args[i]).convert("L"), dtype=np.uint8)
        cand = np.asarray(Image.open(args[i + 1]).convert("L"), dtype=np.uint8)
        r = residue(ref, cand)
        rms = []
        for k in SCALES:
            sm = _box(r, k)
            rms.append(float(np.sqrt((sm**2).mean())))
        share = rms[-1] / rms[0] if rms[0] else 0.0
        name = Path(args[i]).name
        rows.append((name, rms, share))
        print(f"{name[:24]:<26}" + "".join(f"{v:>9.3f}" for v in rms) + f"{share:>10.1%}")

    print("\n读法：**低频占比**＝核 32 的 RMS ÷ 核 4 的 RMS。")
    print("  比值大 → 残留是**成片的**（簇状），放大核也抹不掉，人眼看得见；")
    print("  比值小 → 残留是**细密的**，一放大就没了，看不见。")
    print("\n判据今天只读核 4 那一个数——**两种残留在它眼里只差一个倍数，结构差别读不出来**。")
    return 0


def _box(a: np.ndarray, k: int) -> np.ndarray:
    """k×k 局部均值，f64 进 f64 出，边界最近像素延拓。"""
    pad = k // 2
    p = np.pad(a, pad, mode="edge")
    cs = np.pad(p.cumsum(0).cumsum(1), ((1, 0), (1, 0)))
    h, w = a.shape
    total = cs[k:, k:] - cs[:-k, k:] - cs[k:, :-k] + cs[:-k, :-k]
    return total[:h, :w] / (k * k)


if __name__ == "__main__":
    sys.exit(main())
