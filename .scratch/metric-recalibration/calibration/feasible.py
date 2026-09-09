"""可行域：四条已有的真机约束能不能被同一组数同时满足。

票面《已经有的约束，不要重测》列了四条互相夹着的边界，末一条写着「六个数要同时满足
——满足不了就说明少了一项，当场记下而不是挑一个牺牲掉」。**这一步就是去看它满不满足得了**，
在真机判读之前——判读很贵，先算掉能算的，剩下的才值得上机。

四个数张开搜索空间（**阈值不搜**，它由约束直接夹出来，见 `threshold_window`）：

    颗粒地板比例 grain_ratio · 低通地板比例 low_pass_ratio
    MASKING_FLOOR · MASKING_KNEE

约束：

  B 组四页  2bit+FS 必须落在阈值**之外**（真机零接受）
  C 组八页  2bit+FS 必须仍在阈值**之内**（真机 6 平 2 偏、零个「不收」）
  闸① 四页  判据要与真机逐页同向（3 页偏 2bit+FS、1 页偏 4bit 不抖）
  棋魂两页  **1bit+FS 每页垫底**——同页上它要高过 2bit 不抖（见 `BOTTOM_OF_PAGE`）

**前三条来自票面**《已经有的约束，不要重测》（那一节的第四条是 A 组，票面明写
「归 `06`，不占本票的数」，因此不在这里）。**末一条是票面之外的**，由 `tonefit-c2`
提出，出处是 measurements《位深盲测》——它撑起缺口的一端，但**严版那处不可满足
不依赖它**，去掉它「满足不了」这个结论仍然成立。

头两条一起把阈值夹进 `[max(C 组读数), min(B 组读数))`；这个区间空掉，那组参数就不可行。
闸① 与阈值无关，只比同一页两档的大小（宽版）；严版另要求它**选得到**。

**怎么搜得动**：四个数只从三处进判据——`weight` 只吃掉掩蔽那两个，
低通项与颗粒项各只吃掉自己那道地板。于是固定一对掩蔽参数，
其余两维的**全部**组合可以在同一批块上一次算完（`scores_over_floors`）。

用法：

    python feasible.py <tiles.npz>
"""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from metric_replica import (  # noqa: E402
    TAIL_TILES,
    UPPER_QUANTILE,
    masking_weight,
    quantisation_step,
)

# ── 三组页，逐页对应票面那四条约束 ──────────────────────────────────

B_WHITE = [  # 武器種族傳說 v01 原档 · 纸白 253（离格量 2）· 真机 4/4 判 4bit 不抖更干净
    ("B_武器白底", "MHZ01_088-2.png"),
    ("B_武器白底", "MHZ01_089-1.png"),
    ("B_武器白底", "MHZ01_089-2.png"),
    ("B_武器白底", "MHZ01_090.png"),
]

C_ENOUGH = [  # B 类一手发布版 · 纸白 255（离格量 0）· 真机 6 平 2 偏、零个「不收」
    ("C_N和S_43", "001.png"),
    ("C_N和S_43", "005.png"),
    ("C_N和S_43", "009.png"),
    ("C_N和S_43", "013.png"),
    ("C_Bowing_16", "001.png"),
    ("C_Bowing_16", "004.png"),
    ("C_Bowing_16", "008.png"),
    ("C_Bowing_16", "012.png"),
]

# 闸①：哆啦A梦 v15 连续灰调，AB 交错本第 9~12 对。真机 3:1 偏 2bit+FS。
# 第三项记的是**真机**判更干净的那一档，判据要与它同向。
GATE_ONE = [
    ("G_哆啦A梦闸1", "FCD-DFC-15-1004-冒险游戏书-01-1.png", "2bit+FS"),  # 第 9 对
    ("G_哆啦A梦闸1", "FCD-DFC-15-1046-梦游盒-04-2.png", "4bit"),  # 第 10 对（反向那一对）
    ("G_哆啦A梦闸1", "FCD-DFC-15-0969-列车粉笔-01.png", "2bit+FS"),  # 第 11 对
    ("G_哆啦A梦闸1", "FCD-DFC-15-1000-报仇传票-05-1.png", "2bit+FS"),  # 第 12 对
]

# 第四条：**1bit+FS 每页垫底**。这一条是盲测夹出来的，不是闭式解——measurements
# 《位深盲测》那张表记着「上界再高一点棋魂 0148 上 1bit+FS 就不再高过同页 2bit 不抖」，
# 于是 1bit 上的地板上界是 60（`60/255 = 0.2353`）。它也是 ADR 0002 立颗粒项的原始动机：
# 只有低通项时判据在同一页之内把序排反（棋魂 0060）。
# 约束：同一页上 `1bit+FS` 的读数要**高过** `2bit 不抖`。
BOTTOM_OF_PAGE = [
    ("Q_棋魂垫底", "QH-01_0060.png"),
    ("Q_棋魂垫底", "QH-01_0148.png"),
]

PARAM_NAMES = ["grain_ratio", "low_pass_ratio", "masking_floor", "masking_knee"]


class Tiles:
    """导出的逐块四原始量，按页按档取用。"""

    def __init__(self, path: Path):
        self.z = dict(np.load(path, allow_pickle=False))

    def parts(self, page, depth_label):
        group, name = page
        key = f"{group}|{name}"
        return {
            "depth": int(self.z[f"{key}|{depth_label}|depth"][0]),
            "activity": self.z[f"{key}|activity"],
            "low_pass_error": self.z[f"{key}|{depth_label}|low_pass_error"],
            "grain": self.z[f"{key}|{depth_label}|grain"],
            "ref_grain": self.z[f"{key}|ref_grain"],
        }


def scores_over_floors(parts, weight, grain_ratios, low_pass_ratios) -> np.ndarray:
    """固定掩蔽加权，一次算出两道地板**全部**取值组合下的读数。

    返回形状 `(len(grain_ratios), len(low_pass_ratios))`。
    """
    step = quantisation_step(parts["depth"])
    # (G, 1, tiles) 与 (1, L, tiles) 广播成 (G, L, tiles)
    grain = np.maximum(
        parts["grain"] - parts["ref_grain"] - np.asarray(grain_ratios)[:, None, None] * step, 0.0
    )
    low = np.maximum(
        parts["low_pass_error"] - np.asarray(low_pass_ratios)[None, :, None] * step, 0.0
    )
    return _aggregate(weight * (low + grain))


def _aggregate(values: np.ndarray) -> np.ndarray:
    """沿最后一维聚合：上分位与「第 K 差的那一块」之间更严的那个。

    `metric.rs::aggregate` —— 取的是**那一个秩上的那一块**，不是尾巴的均值。
    """
    n = values.shape[-1]
    by_share = max(int(np.ceil(UPPER_QUANTILE * n)), 1)
    by_count = n + 1 - min(TAIL_TILES, n)
    rank = max(by_share, by_count) - 1
    return np.sort(values, axis=-1)[..., rank]


def sweep(tiles: Tiles, grain_ratios, low_pass_ratios, masking_floors, masking_knees) -> dict:
    """在四维网格上算出三条约束各自的判定，返回逐维取值与逐组结果。

    结果数组的轴序是 `(F, K, G, L)`。
    """
    shape = (len(masking_floors), len(masking_knees), len(grain_ratios), len(low_pass_ratios))
    lo = np.empty(shape)  # C 组读得最高的那一页 → 阈值下界
    hi = np.empty(shape)  # B 组读得最低的那一页 → 阈值上界
    agree = np.zeros(shape, dtype=np.int8)  # 闸① 与真机同向的页数
    lo_page = np.empty(shape, dtype=np.int16)
    hi_page = np.empty(shape, dtype=np.int16)
    # 严版：闸① 不只要「排序同向」，还要**选得到**——选档取的是候选升序里第一个进阈值的
    # （`decide.rs::decide`），排序对而两档都在阈值外时，判定会往更高的档上走，
    # 真机说更干净的那一档仍然选不到。
    lo_strong = np.empty(shape)
    hi_strong = np.empty(shape)
    bottom = np.zeros(shape, dtype=bool)  # 1bit+FS 在两页上都垫底

    cache = {
        page + (label,): tiles.parts(page, label)
        for page in {p for p in B_WHITE + C_ENOUGH} | {(g, n) for g, n, _ in GATE_ONE}
        for label in ("2bit+FS", "4bit")
    }
    cache.update(
        {
            page + (label,): tiles.parts(page, label)
            for page in BOTTOM_OF_PAGE
            for label in ("1bit+FS", "2bit")
        }
    )

    for fi, f in enumerate(masking_floors):
        for ki, k in enumerate(masking_knees):
            weights = {
                key: masking_weight(parts["activity"], f, k) for key, parts in cache.items()
            }

            c = np.stack(
                [
                    scores_over_floors(
                        cache[p + ("2bit+FS",)],
                        weights[p + ("2bit+FS",)],
                        grain_ratios,
                        low_pass_ratios,
                    )
                    for p in C_ENOUGH
                ]
            )
            b = np.stack(
                [
                    scores_over_floors(
                        cache[p + ("2bit+FS",)],
                        weights[p + ("2bit+FS",)],
                        grain_ratios,
                        low_pass_ratios,
                    )
                    for p in B_WHITE
                ]
            )
            lo[fi, ki] = c.max(axis=0)
            hi[fi, ki] = b.min(axis=0)
            lo_page[fi, ki] = c.argmax(axis=0)
            hi_page[fi, ki] = b.argmin(axis=0)

            hits = np.zeros(shape[2:], dtype=np.int8)
            want_in = []  # 真机说 2bit+FS 更干净的页：判据要**选得到**它 → 读数 ≤ 阈值
            want_out = []  # 真机说 4bit 更干净的那一页：2bit+FS 要落在阈值之外
            for group, name, won in GATE_ONE:
                p = (group, name)
                fs = scores_over_floors(
                    cache[p + ("2bit+FS",)], weights[p + ("2bit+FS",)], grain_ratios, low_pass_ratios
                )
                plain = scores_over_floors(
                    cache[p + ("4bit",)], weights[p + ("4bit",)], grain_ratios, low_pass_ratios
                )
                picked_fs = fs < plain
                hits += (picked_fs if won == "2bit+FS" else ~picked_fs).astype(np.int8)
                (want_in if won == "2bit+FS" else want_out).append(fs)
            agree[fi, ki] = hits
            lo_strong[fi, ki] = np.maximum(lo[fi, ki], np.stack(want_in).max(axis=0))
            hi_strong[fi, ki] = np.minimum(hi[fi, ki], np.stack(want_out).min(axis=0))

            still_bottom = np.ones(shape[2:], dtype=bool)
            for p in BOTTOM_OF_PAGE:
                fs1 = scores_over_floors(
                    cache[p + ("1bit+FS",)], weights[p + ("1bit+FS",)], grain_ratios, low_pass_ratios
                )
                plain2 = scores_over_floors(
                    cache[p + ("2bit",)], weights[p + ("2bit",)], grain_ratios, low_pass_ratios
                )
                still_bottom &= fs1 > plain2
            bottom[fi, ki] = still_bottom

    return {
        "轴": (masking_floors, masking_knees, grain_ratios, low_pass_ratios),
        "阈值下界": lo,
        "阈值上界": hi,
        "夹住下界的页": lo_page,
        "夹住上界的页": hi_page,
        "闸①同向": agree,
        "严下界": lo_strong,
        "严上界": hi_strong,
        "垫底仍成立": bottom,
    }


def describe(res: dict, index, strict: bool = False) -> str:
    floors, knees, grains, low_passes = res["轴"]
    fi, ki, gi, li = index
    lo = res["严下界" if strict else "阈值下界"][index]
    hi = res["严上界" if strict else "阈值上界"][index]
    mark = "非空" if lo < hi else "**空**"
    lines = [
        f"  颗粒地板比例 {grains[gi]:.4f} · 低通地板比例 {low_passes[li]:.4f} · "
        f"MASKING_FLOOR {floors[fi]:g} · MASKING_KNEE {knees[ki]:g}",
        f"  阈值窗口 [{lo:.3f}, {hi:.3f})  {mark}"
        f"   下界←{C_ENOUGH[res['夹住下界的页'][index]][1]}"
        f"  上界←{B_WHITE[res['夹住上界的页'][index]][1]}",
        f"  闸① 与真机同向 {res['闸①同向'][index]}/4",
    ]
    return "\n".join(lines)


def main(npz: Path) -> int:
    tiles = Tiles(npz)

    print("=== 今天的判据（src/metric.rs 里的那四个数）===")
    live = sweep(tiles, [0.215_686_27], [0.0], [0.5], [8.0])
    print(describe(live, (0, 0, 0, 0)))
    gate_detail(tiles, 0.215_686_27, 0.0, 0.5, 8.0)

    # 颗粒地板比例搜到 0.55：平坦调上 FS 的高频起伏上界是 `sqrt(u(s−u))`，`u = s/2`
    # 时取到 `s/2`，比例 0.5 就把颗粒项整项吃掉了。搜不到那里就看不见「闸① 要多大的
    # 地板才肯放过连续灰调」，而那正是这一票要夹的东西。
    grain_ratios = np.round(np.arange(0.00, 0.5501, 0.005), 4)
    low_pass_ratios = np.round(np.arange(0.00, 0.3001, 0.01), 4)
    masking_floors = np.round(
        np.concatenate([np.arange(0.02, 0.201, 0.02), np.arange(0.25, 1.001, 0.05)]), 4
    )
    masking_knees = np.round(
        np.concatenate([np.arange(1.0, 10.01, 1.0), np.arange(12.0, 40.01, 4.0)]), 4
    )
    total = len(grain_ratios) * len(low_pass_ratios) * len(masking_floors) * len(masking_knees)
    print(
        f"\n=== 网格搜索：颗粒地板 {len(grain_ratios)} × 低通地板 {len(low_pass_ratios)} × "
        f"FLOOR {len(masking_floors)} × KNEE {len(masking_knees)} = {total} 组 ==="
    )

    res = sweep(tiles, grain_ratios, low_pass_ratios, masking_floors, masking_knees)
    window = res["阈值下界"] < res["阈值上界"]

    print("\n── 宽版：闸① 只要求判据与真机**排序同向** ──")
    ok = window & (res["闸①同向"] == 4)
    print(f"窗口非空 且 闸① 4/4 同向：{int(ok.sum())} / {total} 组")
    if ok.any():
        summarise(res, ok)
        show(tiles, res, _widest(res, ok))

    bottom = res["垫底仍成立"]
    print("\n── 加上第四条：1bit+FS 每页垫底（盲测夹的上界 60，即比例 0.2353）──")
    print(f"单看这一条成立的组合：{int(bottom.sum())} / {total}")
    ok_b = ok & bottom
    print(f"宽版三条 ＋ 垫底：{int(ok_b.sum())} / {total} 组")
    if ok_b.any():
        summarise(res, ok_b)
        show(tiles, res, _widest(res, ok_b))
    else:
        axis = res["轴"][2]
        keep = axis[np.unique(np.nonzero(bottom)[2])] if bottom.any() else []
        need = axis[np.unique(np.nonzero(ok)[2])] if ok.any() else []
        print("\n  **归零。**两条对颗粒地板比例要的是不相交的两段：")
        if len(keep):
            print(f"    垫底还守得住的：{keep.min():.3f} ~ {keep.max():.3f}")
        if len(need):
            print(f"    闸① 要的：      {need.min():.3f} ~ {need.max():.3f}")
        if len(keep) and len(need):
            print(f"    中间空着 {need.min() - keep.max():.3f}")

    print("\n── 严版：真机说 2bit+FS 的三页，判据要**选得到** 2bit+FS ──")
    print("（选档取的是候选升序里第一个进阈值的。排序对而两档都在阈值外，判定仍会往上走。）")
    strong = res["严下界"] < res["严上界"]
    ok2 = strong & (res["闸①同向"] == 4)
    print(f"严版窗口非空 且 闸① 4/4 同向：{int(ok2.sum())} / {total} 组")
    print(f"严版 ＋ 垫底：{int((ok2 & bottom).sum())} / {total} 组")
    if (ok2 & bottom).any():
        summarise(res, ok2 & bottom)
        show(tiles, res, _widest(res, ok2 & bottom, strict=True), strict=True)
        return 0

    print("\n**满足不了**——票面末一条说的正是这种情形：当场记下，不挑一个牺牲掉。")
    if ok.any():
        w = _widest(res, ok)
        gap = res["严下界"][w] - res["严上界"][w]
        print(f"\n宽版最宽的那一组上，严版的窗口倒挂 {gap:.3f}：")
        print(f"  要选得到 2bit+FS，阈值至少 {res['严下界'][w]:.3f}")
        print(f"  要让 B 组落在阈值之外，阈值至多 {res['严上界'][w]:.3f}")
    return 1


def show(tiles: Tiles, res: dict, index, strict: bool = False) -> None:
    print("\n阈值窗口最宽的那一组：")
    print(describe(res, index, strict))
    gate_detail(
        tiles,
        res["轴"][2][index[2]],
        res["轴"][3][index[3]],
        res["轴"][0][index[0]],
        res["轴"][1][index[1]],
    )


def _widest(res: dict, mask: np.ndarray, strict: bool = False):
    """`mask` 圈出的组合里，阈值窗口最宽的那一组。"""
    hi, lo = ("严上界", "严下界") if strict else ("阈值上界", "阈值下界")
    width = np.where(mask, res[hi] - res[lo], -np.inf)
    return np.unravel_index(np.argmax(width), width.shape)


def summarise(res: dict, mask: np.ndarray) -> None:
    axes = res["轴"]
    order = [2, 3, 0, 1]  # 报告按 grain / low_pass / floor / knee 的顺序读
    for name, axis in zip(PARAM_NAMES, order):
        taken = np.unique(np.nonzero(mask)[axis])
        vals = axes[axis][taken]
        print(f"  {name:<16} {vals.min():g} ~ {vals.max():g}   （取到 {len(vals)} 个值）")


def gate_detail(tiles: Tiles, grain_ratio, low_pass_ratio, masking_floor, masking_knee) -> None:
    """闸① 逐页把两档读数印出来——方向对不对，看的是这四行。"""
    for group, name, won in GATE_ONE:
        page = (group, name)
        vals = {}
        for label in ("2bit+FS", "4bit"):
            parts = tiles.parts(page, label)
            weight = masking_weight(parts["activity"], masking_floor, masking_knee)
            vals[label] = float(
                scores_over_floors(parts, weight, [grain_ratio], [low_pass_ratio])[0, 0]
            )
        says = "2bit+FS" if vals["2bit+FS"] < vals["4bit"] else "4bit"
        print(
            f"    {'✓' if says == won else '✗'} {name:<36} "
            f"2bit+FS {vals['2bit+FS']:7.3f} · 4bit {vals['4bit']:7.3f} · 真机 {won}"
        )


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1])))
