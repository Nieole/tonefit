"""可行域：四条已有的真机约束能不能被同一组数同时满足。

票面《已经有的约束，不要重测》列了四条互相夹着的边界，末一条写着「六个数要同时满足
——满足不了就说明少了一项，当场记下而不是挑一个牺牲掉」。**这一步就是去看它满不满足得了**，
在真机判读之前——判读很贵，先算掉能算的，剩下的才值得上机。

四个数张开搜索空间（**阈值不搜**，它由约束直接夹出来，见 `threshold_window_for`）：

    颗粒地板比例 grain_ratio · 低通地板比例 low_pass_ratio
    MASKING_FLOOR · MASKING_KNEE

约束的形态是**逐页的判定档**（见 `WANTED`），不是「某一档的读数在阈值内外」：

  B 组四页  判定不得低于 4bit（真机 4/4 判 4bit 不抖更干净、2bit+FS 零接受）
  C 组八页  判定就该是 2bit+FS（真机 6 平 2 偏、零个「不收」）
  闸① 四页  判定落在真机说更干净的那一档上（3 页 2bit+FS、1 页 ≥4bit）
  棋魂两页  **1bit+FS 每页垫底**——同页上它要高过 2bit 不抖（见 `BOTTOM_OF_PAGE`）

**前三条来自票面**《已经有的约束，不要重测》（那一节的第四条是 A 组，票面明写
「归 `06`，不占本票的数」，因此不在这里）。**末一条是票面之外的**，由 `tonefit-c2`
提出，出处是 measurements《位深盲测》。

**为什么必须按判定档写**：判定取的是候选升序里**第一个进阈值的那一档**
（`decide.rs::decide`）。早先只盯着 `2bit+FS` 的读数在阈值内外，漏掉了「排在它前面的
更低档先进了阈值」——实测漏出过一组「解」，在它上面 B 组四页全判 `1bit`，
而真机在那四页要的是 4bit 不抖。**那种解满足旧写法的每一条，却是坏的。**

每一页因此各给出一个阈值区间（要判成的那一档进得来、前面的都进不来），
全部求交；交集空掉，那组参数就不可行。

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

# 真机结论逐页翻译成「这一页该判成哪一档」。**这才是约束的正确形态**——
# 判定是「候选升序里第一个进阈值的那一档」，只盯着 2bit+FS 的读数在阈值内外，
# 会漏掉「1bit 先进了阈值」这种情形（实测漏过：某组参数下 B 组四页全判 1bit，
# 而真机在那四页上要的是 4bit 不抖）。
WANTED = (
    # B 组：真机 4/4 判 4bit 不抖更干净、2bit+FS 零接受 → 判定不得低于 4bit
    [(p, "≥4bit") for p in B_WHITE]
    # C 组：真机说 2bit+FS 就够（6 平 2 偏、零个「不收」）→ 判定就该是 2bit+FS
    + [(p, "2bit+FS") for p in C_ENOUGH]
    # 闸①：真机说更干净的那一档，就是判定该落的那一档
    + [((g, n), "2bit+FS" if won == "2bit+FS" else "≥4bit") for g, n, won in GATE_ONE]
)

PARAM_NAMES = ["grain_ratio", "low_pass_ratio", "masking_floor", "masking_knee"]

# 候选升序，`Candidate::all` 就是这个次序——选档取的是**第一个进阈值的那一档**
# （`decide.rs::decide`），所以「哪一档被选中」不只看某一档的读数，
# 还看**排在它前面的档有没有先进阈值**。
ASCENDING = ["1bit", "1bit+FS", "2bit", "2bit+FS", "4bit", "4bit+FS"]


def threshold_window_for(reads: dict, wanted: str):
    """要让这一页判成 `wanted`，阈值得落在哪个区间。

    判定 = 候选升序里第一个 `读数 ≤ 阈值` 的档。于是要判成 `wanted`：

        阈值 ≥ 读数[wanted]                    （它自己进得来）
        阈值 <  min(排在它前面那几档的读数)      （前面的都进不来）

    `wanted` 传 `"≥4bit"` 表示「不低于 4bit 即可」——那时只要前四档都进不来，
    选中的是 4bit 还是 4bit+FS 都算合格（都不进就取候选上界，仍是 4bit+FS）。
    """
    if wanted == "≥4bit":
        return -np.inf, min(reads[d] for d in ASCENDING[:4])
    below = ASCENDING[: ASCENDING.index(wanted)]
    return reads[wanted], min((reads[d] for d in below), default=np.inf)


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
    """在四维网格上把每一组参数判一遍，返回逐维取值与逐组结果。

    每一页按 `WANTED` 要它判成的那一档，各解出一个阈值区间，全部求交——
    `阈值下界` / `阈值上界` 就是交集，空掉即不可行；`夹住下界的页` / `夹住上界的页`
    记的是 `WANTED` 里的下标，指出是哪一页把窗口卡死的。

    结果数组的轴序是 `(F, K, G, L)`。
    """
    shape = (len(masking_floors), len(masking_knees), len(grain_ratios), len(low_pass_ratios))
    lo = np.empty(shape)  # 逐页阈值区间求交之后的下界
    hi = np.empty(shape)  # 同上，上界
    agree = np.zeros(shape, dtype=np.int8)  # 闸① 与真机同向的页数
    lo_page = np.empty(shape, dtype=np.int16)
    hi_page = np.empty(shape, dtype=np.int16)
    bottom = np.zeros(shape, dtype=bool)  # 1bit+FS 在两页上都垫底

    # 判定要看**全部六档**，不能只取两档
    every = {p for p, _ in WANTED} | set(BOTTOM_OF_PAGE)
    cache = {
        page + (label,): tiles.parts(page, label) for page in every for label in ASCENDING
    }

    for fi, f in enumerate(masking_floors):
        for ki, k in enumerate(masking_knees):
            weights = {
                key: masking_weight(parts["activity"], f, k) for key, parts in cache.items()
            }

            reads = {
                (p, d): scores_over_floors(
                    cache[p + (d,)], weights[p + (d,)], grain_ratios, low_pass_ratios
                )
                for p in every
                for d in ASCENDING
            }

            # 逐页把「该判成哪一档」翻成阈值区间，再求交
            page_lo = np.full(shape[2:], -np.inf)
            page_hi = np.full(shape[2:], np.inf)
            which_lo = np.zeros(shape[2:], dtype=np.int16)
            which_hi = np.zeros(shape[2:], dtype=np.int16)
            for index, (p, wanted) in enumerate(WANTED):
                r = {d: reads[(p, d)] for d in ASCENDING}
                if wanted == "≥4bit":
                    one_lo = np.full(shape[2:], -np.inf)
                    one_hi = np.minimum.reduce([r[d] for d in ASCENDING[:4]])
                else:
                    below = ASCENDING[: ASCENDING.index(wanted)]
                    one_lo = r[wanted]
                    one_hi = np.minimum.reduce([r[d] for d in below])
                which_lo = np.where(one_lo > page_lo, index, which_lo)
                which_hi = np.where(one_hi < page_hi, index, which_hi)
                page_lo = np.maximum(page_lo, one_lo)
                page_hi = np.minimum(page_hi, one_hi)
            lo[fi, ki] = page_lo
            hi[fi, ki] = page_hi
            lo_page[fi, ki] = which_lo
            hi_page[fi, ki] = which_hi

            hits = np.zeros(shape[2:], dtype=np.int8)
            for group, name, won in GATE_ONE:
                p = (group, name)
                fs, plain = reads[(p, "2bit+FS")], reads[(p, "4bit")]
                picked_fs = fs < plain
                hits += (picked_fs if won == "2bit+FS" else ~picked_fs).astype(np.int8)
            agree[fi, ki] = hits

            still_bottom = np.ones(shape[2:], dtype=bool)
            for p in BOTTOM_OF_PAGE:
                still_bottom &= reads[(p, "1bit+FS")] > reads[(p, "2bit")]
            bottom[fi, ki] = still_bottom

    return {
        "轴": (masking_floors, masking_knees, grain_ratios, low_pass_ratios),
        "阈值下界": lo,
        "阈值上界": hi,
        "夹住下界的页": lo_page,
        "夹住上界的页": hi_page,
        "闸①同向": agree,
        "垫底仍成立": bottom,
    }


def describe(res: dict, index) -> str:
    floors, knees, grains, low_passes = res["轴"]
    fi, ki, gi, li = index
    lo, hi = res["阈值下界"][index], res["阈值上界"][index]
    mark = "非空" if lo < hi else "**空**"
    lo_page, lo_want = WANTED[res["夹住下界的页"][index]]
    hi_page, hi_want = WANTED[res["夹住上界的页"][index]]
    lines = [
        f"  颗粒地板比例 {grains[gi]:.4f} · 低通地板比例 {low_passes[li]:.4f} · "
        f"MASKING_FLOOR {floors[fi]:g} · MASKING_KNEE {knees[ki]:g}",
        f"  阈值窗口 [{lo:.3f}, {hi:.3f})  {mark}",
        f"    下界 ← {lo_page[1]} 要判成 {lo_want}",
        f"    上界 ← {hi_page[1]} 要判成 {hi_want}",
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
    bottom = res["垫底仍成立"]

    print("\n── 三条真机约束：逐页的**判定档**要与真机结论相容 ──")
    print("（不是「某一档的读数在阈值内外」——判定取的是候选升序里第一个进阈值的那一档，")
    print(" 只盯一档会漏掉「更低的档先进了阈值」。）")
    print(f"\n阈值窗口非空：{int(window.sum())} / {total} 组")
    if window.any():
        summarise(res, window)
        show(tiles, res, _widest(res, window))

    print("\n── 加上第四条：1bit+FS 每页垫底 ──")
    print(f"单看这一条成立的：{int(bottom.sum())} / {total} 组")
    both = window & bottom
    print(f"四条同时满足：{int(both.sum())} / {total} 组")
    if both.any():
        summarise(res, both)
        show(tiles, res, _widest(res, both))
        return 0

    print("\n**满足不了**——票面末一条说的正是这种情形：当场记下，不挑一个牺牲掉。")
    if window.any() and bottom.any():
        axis = res["轴"][2]
        keep = axis[np.unique(np.nonzero(bottom)[2])]
        need = axis[np.unique(np.nonzero(window)[2])]
        print("\n  两条对颗粒地板比例要的是不相交的两段：")
        print(f"    垫底还守得住的：  {keep.min():.3f} ~ {keep.max():.3f}")
        print(f"    三条约束要的：    {need.min():.3f} ~ {need.max():.3f}")
        if need.min() > keep.max():
            print(f"    中间空着 {need.min() - keep.max():.3f}")
    return 1


def show(tiles: Tiles, res: dict, index) -> None:
    print("\n阈值窗口最宽的那一组：")
    print(describe(res, index))
    gate_detail(
        tiles,
        res["轴"][2][index[2]],
        res["轴"][3][index[3]],
        res["轴"][0][index[0]],
        res["轴"][1][index[1]],
    )


def _widest(res: dict, mask: np.ndarray):
    """`mask` 圈出的组合里，阈值窗口最宽的那一组。"""
    width = np.where(mask, res["阈值上界"] - res["阈值下界"], -np.inf)
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
