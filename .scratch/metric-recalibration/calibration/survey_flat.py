"""普查：每一页上最大的那块**没有文字的平坦调**，连同它的纸白与《离格量》。

`04` 要的「平坦调专用裁块」由它挑页——真机 D 组没测干净，就是因为那四页上
平坦白底与文字两件事同时在打架（文字被就近取整推淡，判读者选抖动的两次都是因为文字）。
这一趟要的裁块里**不能有文字**。

判「没有文字」不靠严格平坦（扫描的白底本来就有噪声，严格平坦区往往只有几千像素），
靠的是**这块里最暗的像素有多暗**：笔画与线稿一进来，暗端立刻掉到 200 以下。

用法：

    python survey_flat.py <参照 8bit 的目录或 cbz> [--side 512] [--top 8]
"""

import argparse
import io
import sys
import zipfile
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from metric_replica import offgrid  # noqa: E402,F401  （《离格量》的单一实现，转出去给别处用）

# 一块「白底」要满足的两条：暗端不低于这个（没有笔画），且绝大多数像素在纸白附近。
DARKEST_ALLOWED = 200
NEAR_WHITE = 240
NEAR_WHITE_SHARE = 0.98


def paper_white(image: np.ndarray) -> int:
    """纸白：最大那块严格平坦区（3×3 邻域方差为 0）里的众数灰度。

    与 measurements 的《全语料普查：四成三的页纸白不落在格点上》同一条定义。
    """
    flat = local_variance(image) < 1e-9
    if flat.sum() < 5000:
        return -1
    values = image[flat]
    return int(np.bincount(values).argmax())


def local_variance(image: np.ndarray, k: int = 3) -> np.ndarray:
    """k×k 邻域方差。「严格平坦」就是它为零的地方。"""
    f = image.astype(np.float64)
    mean = box_mean(f, k)
    return np.maximum(box_mean(f * f, k) - mean * mean, 0.0)


def box_mean(a: np.ndarray, k: int) -> np.ndarray:
    """k×k 的局部均值，边界最近像素延拓。

    与 `metric_replica.low_pass` **不是**同一件事：那一支要与 `src/metric.rs` 逐位一致，
    偶数核的窗口左右差一格；这一支是普查用的对称窗口，两者不可互换。
    """
    pad = k // 2
    p = np.pad(a, pad, mode="edge")
    cs = p.cumsum(0).cumsum(1)
    cs = np.pad(cs, ((1, 0), (1, 0)))
    h, w = a.shape
    total = cs[k:, k:] - cs[:-k, k:] - cs[k:, :-k] + cs[:-k, :-k]
    return total[:h, :w] / (k * k)


def best_flat_block(image: np.ndarray, side: int):
    """找一块 `side`×`side` 的白底：没有笔画，且几乎全在纸白附近。

    积分图上一次扫完所有位置，取「近白像素最多」的那一块。
    """
    h, w = image.shape
    if h < side or w < side:
        return None
    dark = (image < DARKEST_ALLOWED).astype(np.int64)
    near = (image >= NEAR_WHITE).astype(np.int64)
    dark_sum = _rect_sums(dark, side)
    near_sum = _rect_sums(near, side)
    area = side * side
    ok = dark_sum == 0
    if not ok.any():
        return None
    score = np.where(ok, near_sum, -1)
    idx = int(np.argmax(score))
    y, x = divmod(idx, score.shape[1])
    share = near_sum[y, x] / area
    if share < NEAR_WHITE_SHARE:
        return None
    return {"y": int(y), "x": int(x), "side": side, "近白占比": float(share)}


def _rect_sums(a: np.ndarray, side: int) -> np.ndarray:
    """所有 `side`×`side` 窗口的和。"""
    cs = np.pad(a.cumsum(0).cumsum(1), ((1, 0), (1, 0)))
    return cs[side:, side:] - cs[:-side, side:] - cs[side:, :-side] + cs[:-side, :-side]


def pages(source: Path, per: int | None = None):
    """走一个归档或目录里的页。`per` 给了就均匀抽这么多页，否则全走。"""
    if source.is_dir():
        items = sorted(source.rglob("*.png")) + sorted(source.rglob("*.jpg"))
        read = lambda item: item.read_bytes()  # noqa: E731
        name_of = lambda item: item.name  # noqa: E731
    else:
        z = zipfile.ZipFile(source)
        items = sorted(n for n in z.namelist() if not n.endswith("/"))
        read = z.read
        name_of = lambda item: Path(item).name  # noqa: E731
    if per:
        items = items[:: max(1, len(items) // per)][:per]
    for item in items:
        image = Image.open(io.BytesIO(read(item))).convert("L")
        yield name_of(item), np.asarray(image, dtype=np.uint8)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("source", type=Path)
    ap.add_argument("--side", type=int, default=512)
    ap.add_argument("--top", type=int, default=8)
    args = ap.parse_args()

    rows = []
    for name, image in pages(args.source):
        white = paper_white(image)
        if white < 200:
            continue
        block = best_flat_block(image, args.side)
        if block is None:
            continue
        rows.append((name, image.shape, white, offgrid(white, 2), offgrid(white, 4), block))

    rows.sort(key=lambda r: -r[5]["近白占比"])
    print(f"{'页':<34}{'尺寸':<13}{'纸白':>5}{'离格2bit':>9}{'离格4bit':>9}{'近白占比':>9}  位置")
    for name, shape, white, u2, u4, b in rows[: args.top]:
        print(
            f"{name:<34}{shape[1]}x{shape[0]:<8}{white:>5}{u2:>9.0f}{u4:>9.0f}"
            f"{b['近白占比']:>9.3f}  ({b['x']},{b['y']})+{b['side']}"
        )
    print(f"\n{len(rows)} 页有 {args.side}x{args.side} 的无文字白底")


if __name__ == "__main__":
    main()
