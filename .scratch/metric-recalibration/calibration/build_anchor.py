"""造「生态锚点」：从闸① 那四页真实的平滑渐变里裁块，与平移出来的 `u=42` 并排判。

阶梯是**平移**出来的（一块纸底整体加一个常数），它的口径要有东西验证：

- 两者判读一致 → 平移这个口径被验证，阶梯的结论可以直接用。
- 不一致 → **那本身就是发现**，说明平移改变了别的东西（比如把纸的纤维噪声一起
  搬到了中灰上，而真实的中灰渐变没有那种噪声）。

闸① 那四页真实的连续灰调本来就落在 `s/2` 附近（实测被聚合取到那一块的颗粒 RMS
38.3~42.8，而 `s/2 = 42.5`）——中灰在 2bit 上就是长这样，不是造出来的情形。

**挑块的三条**：够平滑（不含线稿与文字）、够接近格点中点（`u` 大）、块内没有强边缘。

用法：

    python build_anchor.py <闸①参照页目录> <出口目录> [--side 512]
"""

import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from material import tile_to_panel  # noqa: E402
from metric_replica import offgrid  # noqa: E402
from survey_flat import local_variance  # noqa: E402


def best_gradient_block(image: np.ndarray, side: int):
    """找一块最像「连续灰调」的：平滑、离格点远、没有强边缘。

    步长取 `side // 4`，够密而不必逐像素滑窗。**不强求 `u` 落在 42**——
    真实页上的连续灰调，块均值到格点的距离本来就散在各处；把每块真实的 `u`
    如实记下来，锚点反而覆盖了阶梯 8~42 之间的空隙。
    """
    h, w = image.shape
    if h < side or w < side:
        return None
    var = local_variance(image)
    best = None
    for y in range(0, h - side + 1, side // 4):
        for x in range(0, w - side + 1, side // 4):
            block = image[y : y + side, x : x + side]
            v = var[y : y + side, x : x + side]
            smooth = float(np.median(v))
            edges = float((v > 400).mean())  # 线稿与文字边缘
            mean = float(block.mean())
            u = offgrid(mean)
            if edges > 0.05 or smooth > 60:
                continue
            score = u - 40 * edges
            if best is None or score > best["score"]:
                best = {
                    "score": score,
                    "y": y,
                    "x": x,
                    "均值": round(mean, 1),
                    "离格量": round(u, 1),
                    "平滑度(3x3方差中位)": round(smooth, 1),
                    "强边缘占比": round(edges, 4),
                    "块内标准差": round(float(block.std()), 1),
                    # 真实渐变里《离格量》逐像素都不同——阶梯上它是常数，这里不是。
                    # 锚点因此验的是「受控平坦调的结论能不能外推到真实连续灰调」。
                    "逐像素离格量中位": round(
                        float(np.median([offgrid(v) for v in block.ravel()[::37]])), 1
                    ),
                }
    return best


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("pages", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--side", type=int, default=384)
    args = ap.parse_args()

    dest = args.out / "页"
    dest.mkdir(parents=True, exist_ok=True)
    found = []
    for page in sorted(args.pages.glob("*.png")):
        image = np.asarray(Image.open(page).convert("L"), dtype=np.uint8)
        block = best_gradient_block(image, args.side)
        if block is None:
            print(f"  {page.name:<40} 没有够平滑的块")
            continue
        crop = image[block["y"] : block["y"] + args.side, block["x"] : block["x"] + args.side]
        name = f"anchor_{page.stem[-12:]}.png"
        Image.fromarray(tile_to_panel(crop), mode="L").save(dest / name)
        found.append({"页": name, "源页": page.name, **{k: v for k, v in block.items() if k != "score"}})
        print(
            f"  {page.name:<40} 均值 {block['均值']:6.1f} · 离格 {block['离格量']:4.1f}"
            f" · 平滑度 {block['平滑度(3x3方差中位)']:5.1f} · 边缘 {block['强边缘占比']:.3%}"
            f" · ({block['x']},{block['y']})+{args.side}"
        )

    (args.out / "锚点.json").write_text(
        json.dumps(
            {
                "是什么": (
                    "闸① 那四页真实平滑渐变的裁块，铺满整页。与阶梯里平移出来的 u=42 "
                    "三格并排判——两者一致则平移那个口径被验证，不一致本身就是发现。"
                ),
                "源": "哆啦A梦 v15（一手 8K 源）· AB 交错本第 9~12 对的那四页",
                "块": found,
            },
            ensure_ascii=False,
            indent=1,
        ),
        encoding="utf-8",
    )
    print(f"\n{len(found)} 块 → {dest}")


if __name__ == "__main__":
    main()
