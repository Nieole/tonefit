"""诊断：闸① 满足不了，缺口有多大、缺在哪一项。

`feasible.py` 的结论是「三条约束满足不了」。**票面末一条要的不是挑一个牺牲掉，
是当场记下少了哪一项**——这个脚本量的就是那个缺口。

三件事：

1. **把一页的读数拆成两项**，各自单独聚合。判据是 `weight × (低通项 + 颗粒项)`，
   而聚合取的是某一个秩上的那一块、不是线性可加，所以两项只能各聚合一次分开看，
   不能相减。
2. **颗粒项即使整项归零，闸① 救不救得回来**——把颗粒地板推到吃掉整项，
   看剩下的低通项够不够翻过来。这是「调地板」这条路的上限。
3. **掩蔽能改的只有「哪一块被取到」**：`masking_weight` 只由参照定，同一页两档拿到的是
   **同一组逐块权重**（`metric.rs::masking_weight` 的注释）。因此**在同一块上**它改不了
   哪一档更差——两侧同乘一个正数。但权重逐块不同，而聚合取的是某个秩上的那一块，
   所以整页读数的大小关系**原则上仍可能被它翻过来**。翻不翻得过来只能实测，
   不能从「同乘」推出来（`feasible.py` 把掩蔽那两维扫满了，闸① 一格没救回来）。

用法：

    python diagnose.py <tiles.npz>
"""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from feasible import B_WHITE, C_ENOUGH, GATE_ONE, Tiles, _aggregate  # noqa: E402
from metric_replica import LIVE, masking_weight, quantisation_step  # noqa: E402


def terms(tiles: Tiles, page, label, **params):
    """一页一档：两项各自单独聚合，外加合起来的那个读数。"""
    parts = tiles.parts(page, label)
    step = quantisation_step(parts["depth"])
    weight = masking_weight(parts["activity"], params["masking_floor"], params["masking_knee"])
    low = np.maximum(parts["low_pass_error"] - params["low_pass_ratio"] * step, 0.0)
    grain = np.maximum(
        parts["grain"] - parts["ref_grain"] - params["grain_ratio"] * step, 0.0
    )
    return {
        "低通项": float(_aggregate(weight * low)),
        "颗粒项": float(_aggregate(weight * grain)),
        "合计": float(_aggregate(weight * (low + grain))),
    }


def table(tiles: Tiles, title: str, pages, labels, **params) -> None:
    print(f"\n=== {title} ===")
    print(f"{'页':<36}{'档':<9}{'低通项':>9}{'颗粒项':>9}{'合计':>9}")
    for page in pages:
        for label in labels:
            t = terms(tiles, page, label, **params)
            print(
                f"{page[1]:<36}{label:<9}"
                f"{t['低通项']:9.3f}{t['颗粒项']:9.3f}{t['合计']:9.3f}"
            )


def main(npz: Path) -> int:
    tiles = Tiles(npz)

    print("今天的四个数：颗粒地板比例 0.2157 · 低通地板比例 0（低通项是裸 RMS）")
    print("            MASKING_FLOOR 0.5 · MASKING_KNEE 8.0")

    table(tiles, "闸①（真机 3:1 偏 2bit+FS）", [(g, n) for g, n, _ in GATE_ONE],
          ["2bit+FS", "4bit"], **LIVE)
    table(tiles, "B 组白底（真机 4/4 偏 4bit 不抖）", B_WHITE, ["2bit+FS", "4bit"], **LIVE)
    table(tiles, "C 组（真机 6 平 2 偏、零个「不收」）", C_ENOUGH, ["2bit+FS", "4bit"], **LIVE)

    print("\n=== 颗粒项整项归零之后，闸① 能不能翻过来 ===")
    print("（颗粒地板推到 1.0×格点间距，颗粒项在三档上都被完全吃掉——这是「调地板」这条路的上限）")
    killed = dict(LIVE, grain_ratio=1.0)
    print(f"\n{'页':<36}{'2bit+FS':>10}{'4bit':>10}{'真机':>10}{'':>4}")
    for group, name, won in GATE_ONE:
        page = (group, name)
        fs = terms(tiles, page, "2bit+FS", **killed)["合计"]
        plain = terms(tiles, page, "4bit", **killed)["合计"]
        says = "2bit+FS" if fs < plain else "4bit"
        print(f"{name:<36}{fs:10.3f}{plain:10.3f}{won:>10}{'  ✓' if says == won else '  ✗'}")

    print("\n=== 掩蔽：逐块权重与候选无关 ===")
    print("（`metric.rs::masking_weight` 只吃参照的活动度。同一页两档拿到的是同一组权重，")
    print(" 因此**在同一块上**它改不了哪一档更差；但权重逐块不同、聚合又取某个秩上的那一块，")
    print(" 整页读数的大小关系原则上仍可能被它翻过来——翻不翻得过来要实测，推不出来。）")
    page = (GATE_ONE[0][0], GATE_ONE[0][1])
    a = tiles.parts(page, "2bit+FS")["activity"]
    b = tiles.parts(page, "4bit")["activity"]
    print(f"\n  {page[1]}：两档的活动度数组逐块相同 → {np.array_equal(a, b)}")

    print("\n=== 逐块看：闸① 那一页上，2bit+FS 比 4bit 差多少块 ===")
    for group, name, won in GATE_ONE:
        p = (group, name)
        fs = tiles.parts(p, "2bit+FS")
        pl = tiles.parts(p, "4bit")
        step_fs, step_pl = quantisation_step(fs["depth"]), quantisation_step(pl["depth"])
        e_fs = np.maximum(fs["low_pass_error"], 0) + np.maximum(
            fs["grain"] - fs["ref_grain"] - LIVE["grain_ratio"] * step_fs, 0
        )
        e_pl = np.maximum(pl["low_pass_error"], 0) + np.maximum(
            pl["grain"] - pl["ref_grain"] - LIVE["grain_ratio"] * step_pl, 0
        )
        worse = (e_fs > e_pl).mean()
        print(
            f"  {name:<36} 2bit+FS 更差的块占 {worse:6.1%}"
            f"  ·  逐块比值中位数 {np.median(e_fs / np.maximum(e_pl, 1e-9)):5.2f}×"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1])))
