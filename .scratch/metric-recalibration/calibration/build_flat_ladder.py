"""造「平坦调阶梯」：把《离格量》与背景亮度**交叉**摆开的一组真机判读材料。

这一组是 `04` 的重心。它一次答三件事：

1. **颗粒地板的下界**——起伏低到什么程度真的看不见，闭式解答不了、只能问眼睛。
2. **平坦区上抖动会不会输给同档不抖动**——票面并进来的那次测量。真机 D 组
   （2:2 平）没测干净：那四页上平坦白底与文字**同时**在被量化，判读者选抖动的
   两次是因为文字浓度、选不抖的两次是因为背景。这一组里**没有文字**。
3. **「稀疏而响」与「密而匀」哪个先被看见**——B 组白底在 `u≈2`（2.4% 的像素掉 83），
   闸① 连续灰调在 `u≈42`（半数像素偏 ±42），后者 RMS 大三倍而真机反而接受。

## 为什么要交叉，不是只扫离格量

直接把一块纸底一路平移到 `u = s/2`，《离格量》与背景亮度会一起变，测出来分不清
是稀疏度变了还是底变暗了。而 2bit 的格点是套嵌的（`0/85/170/255`），**同一个 `u`
在近白、中灰、偏暗三处都取得到**——交叉摆开，两个变量就分开了。

## 口径要交代的一条

像素全部来自一手素材（武器種族傳說 v01 原档的无文字白底），**只动直流**：
整块加一个常数，噪声幅度与纹理原样保留。但平移之后它**不再是那一页的纸白**，
这一条要随材料一同记进 `docs/measurements.md`。

用法：

    python build_flat_ladder.py <参照8bit cbz> <出口目录> [--side 512]
"""

import argparse
import io
import json
import sys
import zipfile
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from material import PANEL, tile_to_panel  # noqa: E402
from metric_replica import offgrid, quantisation_step  # noqa: E402
from survey_flat import best_flat_block, paper_white  # noqa: E402

STEP_2BIT = quantisation_step(2)  # 2bit 的《格点间距》：85，格点 0/85/170/255

# 《离格量》× 背景亮度，十五格。每一列的三个取值离各自最近的 2bit 格点恰好一样远。
#
# `u=1` 是票面并进来的那次测量——「离格量为 1 的那一档没测过」：FS 在白底上撒
# `1/85 = 1.2%`，是 B 组那四页（2.4%）的一半。在阶梯上它是干净的一行，
# 而在真实页上（N和S 第 23 话起纸白 254）还混着页面内容那一项。
# `u=42` 是另一端：`s/2` 上 FS 的高频起伏取到理论最大值 42.5，闸① 那四页正落在这里。
LADDER = {
    1: {"近白": 254, "中灰": 171, "偏暗": 86},
    2: {"近白": 253, "中灰": 172, "偏暗": 87},
    8: {"近白": 247, "中灰": 178, "偏暗": 93},
    21: {"近白": 234, "中灰": 191, "偏暗": 106},
    42: {"近白": 213, "中灰": 128, "偏暗": 43},
}


def pick_block(cbz: Path, side: int):
    """从参照 8bit 里挑一块最干净的无文字白底。

    判「没有文字」靠暗端：笔画一进来，块内最暗的像素立刻掉到 200 以下。
    """
    z = zipfile.ZipFile(cbz)
    best = None
    for name in sorted(n for n in z.namelist() if not n.endswith("/")):
        image = np.asarray(Image.open(io.BytesIO(z.read(name))).convert("L"), dtype=np.uint8)
        white = paper_white(image)
        if white < 200:
            continue
        block = best_flat_block(image, side)
        if block is None:
            continue
        crop = image[block["y"] : block["y"] + side, block["x"] : block["x"] + side]
        cand = {"页": Path(name).name, "纸白": white, "块": block, "像素": crop}
        if best is None or block["近白占比"] > best["块"]["近白占比"]:
            best = cand
    return best


def shift_to(block: np.ndarray, white: int, target: int) -> np.ndarray:
    """把这一块的纸白平移到 `target`，只动直流。"""
    return np.clip(block.astype(np.int16) + (target - white), 0, 255).astype(np.uint8)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("cbz", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--side", type=int, default=512)
    args = ap.parse_args()

    source = pick_block(args.cbz, args.side)
    assert source is not None, "找不到无文字白底"
    print(
        f"取块：{source['页']}  纸白 {source['纸白']}（离格 {offgrid(source['纸白'])}）"
        f"  ({source['块']['x']},{source['块']['y']})+{args.side}"
        f"  近白占比 {source['块']['近白占比']:.3f}"
    )
    lo, hi = int(source["像素"].min()), int(source["像素"].max())
    print(f"     块内灰度 {lo}~{hi}（{hi - lo} 级起伏，那是纸张纹理与扫描噪声）")

    pages = args.out / "页"
    pages.mkdir(parents=True, exist_ok=True)
    manifest = []
    for u, row in LADDER.items():
        for tone, target in row.items():
            assert offgrid(target) == u, f"{target} 离格 {offgrid(target)}，不是 {u}"
            shifted = shift_to(source["像素"], source["纸白"], target)
            panel = tile_to_panel(shifted)
            name = f"u{u:02d}_{tone}_{target}.png"
            Image.fromarray(panel, mode="L").save(pages / name)
            fs_share = u / STEP_2BIT
            manifest.append(
                {
                    "格": f"u{u:02d}_{tone}",
                    "页": name,
                    "离格量": u,
                    "背景": tone,
                    "目标灰度": target,
                    "FS在这块上要撒的点": round(fs_share, 4),
                    "FS高频起伏(sqrt(u(s-u)))": round(float(np.sqrt(u * (STEP_2BIT - u))), 2),
                }
            )
            print(
                f"  u={u:2d} {tone} → {target:3d}   撒点 {fs_share:6.1%}"
                f"   起伏 {np.sqrt(u * (STEP_2BIT - u)):5.2f}"
            )

    (args.out / "阶梯.json").write_text(
        json.dumps(
            {
                "源": {
                    "作品": "武器種族傳說 v01 原档（一手素材）",
                    "页": source["页"],
                    "纸白": source["纸白"],
                    "裁块": f"({source['块']['x']},{source['块']['y']})+{args.side}",
                    "近白占比": round(source["块"]["近白占比"], 4),
                    "块内灰度": [lo, hi],
                },
                "口径": (
                    "像素全部来自一手素材的无文字白底，**只动直流**（整块加一个常数），"
                    "噪声幅度与纹理原样保留；平移之后它不再是那一页的纸白。"
                    f"一块 {args.side}×{args.side} 平铺满 {PANEL[0]}×{PANEL[1]}，"
                    "抖动在拼好之后做，误差扩散在整页上连续。"
                ),
                "格": manifest,
            },
            ensure_ascii=False,
            indent=1,
        ),
        encoding="utf-8",
    )
    print(f"\n{len(manifest)} 格已写入 {pages}")
    print(f"源出处与每格的属性 → {args.out / '阶梯.json'}（编排不在这里，见 pack_device_bundle.arrange）")


if __name__ == "__main__":
    main()
