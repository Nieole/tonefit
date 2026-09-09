"""判据的 Python 复现：把每一块的**四个原始量**导出来。

为什么要复现：`04` 要在五个数（颗粒地板比例、低通地板比例、`MASKING_FLOOR`、
`MASKING_KNEE`、阈值）张开的空间上搜可行域，而那五个数在 `src/metric.rs` 里是
硬编码常数。一块的读数是

    weight(activity) × ( max(低通项 − 低通地板, 0) + max(候选颗粒 − 参照颗粒 − 颗粒地板, 0) )

四个原始量（`低通项` `候选颗粒` `参照颗粒` `活动度`）**与那五个数无关**。
导出一次，任意参数组合都能在它们上面重算，不必为每一组参数跑一遍判据。

**口径自检**：本模块算出的整页读数要与 `tonefit --dry-run` 的那一列对得上
（`export_tiles.py`当场做）。对不上就先查口径——别拿一把没对准的尺子去搜可行域。
这一条是 judgements/README 那条自检要求的同一条。

候选图**不在这里量化**——直接用 tonefit 渲出来的那一份，少一层「Python 量化与
`quantize.rs` 等不等价」的风险。与 `src/metric.rs` 的对应关系逐处写在各函数的注释里。
"""

import numpy as np

# ── 面板与判据的形状常数（`src/metric.rs`）──────────────────────────

VIEWING_DISTANCE_MM = 300.0
KERNEL_ARC_MINUTES = 4.0
MM_PER_INCH = 25.4
KERNEL_RANGE = (2, 4)

TILE = 32
STRUCTURE_KERNEL = TILE
UPPER_QUANTILE = 0.99
TAIL_TILES = 8

DEPTH_LEVELS = {1: 2, 2: 4, 4: 16, 8: 256}

# 今天在 `src/metric.rs` 里的那四个取值。自检与「现状」那一列都引它，不各写一份。
LIVE = dict(grain_ratio=0.215_686_27, low_pass_ratio=0.0, masking_floor=0.5, masking_knee=8.0)


def low_pass_kernel(ppi: int) -> int:
    """低通核边长，由面板 PPI 推出。`metric.rs::low_pass_kernel`。"""
    span_mm = VIEWING_DISTANCE_MM * np.tan(np.radians(KERNEL_ARC_MINUTES / 60.0))
    pixels = ppi * span_mm / MM_PER_INCH
    return int(np.clip(round(pixels), *KERNEL_RANGE))


def quantisation_step(depth: int) -> float:
    """这一档的《格点间距》：`255 / (2ⁿ − 1)`。`metric.rs::quantisation_step`。"""
    return 255.0 / (DEPTH_LEVELS[depth] - 1)


def grid(depth: int) -> tuple:
    """这一档的格点。格点是套嵌的（`255 = 3×85 = 15×17`，见 `src/quantize.rs`）。"""
    step = quantisation_step(depth)
    return tuple(round(i * step) for i in range(DEPTH_LEVELS[depth]))


def offgrid(value: float, depth: int = 2) -> float:
    """《离格量》：一个取值到这一档最近格点的距离。

    默认 2bit——本轮要夹的都是 2bit 那一档，而 2bit 的格点是 4bit 的子集。
    """
    return min(abs(value - g) for g in grid(depth))


# ── 低通（`src/metric.rs::low_pass`）────────────────────────────────


def low_pass(image: np.ndarray, kernel: int) -> np.ndarray:
    """`kernel`×`kernel` 的局部均值，边界按最近像素延拓，两趟可分离。

    核边长是偶数时窗口无法严格居中，左右差一格——`before` / `after` 的取法与
    Rust 一致。累加在 f64 里走（Rust 也是），最后落回 f32。
    """
    before = (kernel - 1) // 2
    after = (kernel - 1) - before
    rows = _sliding_sum(image.astype(np.float64), before, after, axis=1)
    cols = _sliding_sum(rows, before, after, axis=0)
    return (cols / float(kernel * kernel)).astype(np.float32)


def _sliding_sum(a: np.ndarray, before: int, after: int, axis: int) -> np.ndarray:
    """沿 `axis` 的窗口求和，边界最近像素延拓。"""
    pad = [(0, 0), (0, 0)]
    pad[axis] = (before, after)
    padded = np.pad(a, pad, mode="edge")
    cs = np.cumsum(padded, axis=axis)
    zero = np.zeros_like(np.take(cs, [0], axis=axis))
    cs = np.concatenate([zero, cs], axis=axis)
    n = a.shape[axis]
    width = before + after + 1
    hi = np.take(cs, np.arange(width, width + n), axis=axis)
    lo = np.take(cs, np.arange(0, n), axis=axis)
    return hi - lo


# ── 逐块的四个原始量 ────────────────────────────────────────────────


def tile_slices(height: int, width: int):
    """铺满整页的分块，行优先，边上不足一块的按实际像素数算。`metric.rs::tiles`。"""
    for y in range(0, height, TILE):
        h = min(TILE, height - y)
        for x in range(0, width, TILE):
            w = min(TILE, width - x)
            yield y, x, h, w


def _tile_means(values: np.ndarray, height: int, width: int) -> np.ndarray:
    """把逐像素的量按块求平均，返回行优先的一维数组。"""
    out = []
    for y, x, h, w in tile_slices(height, width):
        out.append(values[y : y + h, x : x + w].mean())
    return np.asarray(out, dtype=np.float64)


def reference_side(image: np.ndarray, ppi: int) -> dict:
    """参照那一侧：低通、参照颗粒、活动度。一张参照只算一次。

    掩蔽加权**只由参照定**（`metric.rs::masking_weight` 的注释），活动度因此在这里出。
    """
    kernel = low_pass_kernel(ppi)
    height, width = image.shape
    lp = low_pass(image, kernel)
    structure = low_pass(image, STRUCTURE_KERNEL)
    f = image.astype(np.float32)
    grain = np.sqrt(_tile_means((f - lp) ** 2, height, width))
    activity = _tile_means(np.abs(f - structure), height, width)
    return {"kernel": kernel, "low_pass": lp, "grain": grain, "activity": activity}


def candidate_side(candidate: np.ndarray, ref: dict) -> dict:
    """候选那一侧：低通项与候选颗粒。每页每候选各一份。"""
    height, width = candidate.shape
    lp = low_pass(candidate, ref["kernel"])
    f = candidate.astype(np.float32)
    low_pass_error = np.sqrt(_tile_means((ref["low_pass"] - lp) ** 2, height, width))
    grain = np.sqrt(_tile_means((f - lp) ** 2, height, width))
    return {"low_pass_error": low_pass_error, "grain": grain}


# ── 由四个原始量重算一页的读数 ──────────────────────────────────────


def masking_weight(activity: np.ndarray, floor: float, knee: float) -> np.ndarray:
    """一块的对比度掩蔽加权。`metric.rs::masking_weight`。"""
    return floor + (1.0 - floor) * knee / (knee + activity)


def page_score(
    ref: dict,
    cand: dict,
    depth: int,
    grain_ratio: float,
    low_pass_ratio: float,
    masking_floor: float,
    masking_knee: float,
) -> float:
    """一页的判据读数。

    `low_pass_ratio` 是 `04` 要标的那个**新**数——低通项的可见度地板，与颗粒项那道
    地板同型，也是《格点间距》的一个比例。取 0 就退回今天的判据（低通项是裸 RMS）。
    """
    step = quantisation_step(depth)
    weight = masking_weight(ref["activity"], masking_floor, masking_knee)
    low = np.maximum(cand["low_pass_error"] - low_pass_ratio * step, 0.0)
    grain = np.maximum(cand["grain"] - ref["grain"] - grain_ratio * step, 0.0)
    return aggregate(weight * (low + grain))


def aggregate(values: np.ndarray) -> float:
    """分块读数收成一个数：上分位与「第 K 差的那一块」之间更严的那个。

    `metric.rs::aggregate` —— 取的是**那一个秩上的那一块**，不是尾巴的均值。
    参数一变排序就变、取到的块也会变，所以可行域搜索必须留着全部块。
    """
    if values.size == 0:
        return 0.0
    ordered = np.sort(values)
    n = ordered.size
    by_share = max(int(np.ceil(UPPER_QUANTILE * n)), 1)
    by_count = n + 1 - min(TAIL_TILES, n)
    return float(ordered[max(by_share, by_count) - 1])
