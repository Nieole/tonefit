"""给闸① 补测选 12 页：按连续灰调面积分层，每层取几页。

**为什么按面积分层**：「连续灰调」正是这一组要测的素材类别，面积是它的定义量。
按判据读数挑等于用判据给自己出题——挑出来的会是「判据觉得有争议的页」，
那样的结果说不了「真机在连续灰调上普遍偏哪一档」。

原闸① 那四页的面积是 31.1% / 34.8% / 48.6% / 51.9%（真机 3:1 偏 `2bit+FS`，
判 `4bit` 的那一页面积反而最大）。**面积不决定答案**，正好适合当分层轴：
补测覆盖 30%~70%，四层各三页，与原四页的范围重叠并往上扩。

三条排除：

- **原来那四页**——已经判过，不重判（新的 12 对与它们合并成 n=16）
- **非正文页**（目录、封面、初次刊载）——不是漫画页
- **同一话最多两页**——一话里连着取会把那一话的画风当成普遍现象

用法：

    python pick_gate_one.py <参照8bit 目录> <出口 json> [--per-band 3]
"""

import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from survey_continuous_tone import continuous_tone_share  # noqa: E402

ALREADY_JUDGED = {
    "FCD-DFC-15-1004-冒险游戏书-01-1.png",
    "FCD-DFC-15-1046-梦游盒-04-2.png",
    "FCD-DFC-15-0969-列车粉笔-01.png",
    "FCD-DFC-15-1000-报仇传票-05-1.png",
}
BANDS = [(0.30, 0.40), (0.40, 0.50), (0.50, 0.60), (0.60, 0.70)]
MAX_PER_STORY = 2
SEED = 20260910


def story_of(name: str) -> str:
    """页名里那一话的编号，例如 `FCD-DFC-15-1004-冒险游戏书-01-1.png` → `1004`。"""
    m = re.search(r"FCD-DFC-15-(\d+)-", name)
    return m.group(1) if m else name


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("pages", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--per-band", type=int, default=3)
    args = ap.parse_args()

    cache = args.out.with_suffix(".面积.json")
    if cache.exists():
        shares = json.load(open(cache, encoding="utf-8"))
        print(f"读缓存：{len(shares)} 页")
    else:
        shares = {}
        for i, page in enumerate(sorted(args.pages.rglob("*.png")), 1):
            image = np.asarray(Image.open(page).convert("L"), dtype=np.uint8)
            shares[page.name] = continuous_tone_share(image)
            if i % 100 == 0:
                print(f"  量了 {i} 页…")
        json.dump(shares, open(cache, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
        print(f"量完 {len(shares)} 页，缓存 → {cache}")

    ok = {
        n: s
        for n, s in shares.items()
        if n not in ALREADY_JUDGED and "-0000-" not in n
    }
    print(f"\n排除已判过的 4 页与非正文页之后，剩 {len(ok)} 页\n")

    rng = np.random.default_rng(SEED)
    picked, used = [], defaultdict(int)
    for lo, hi in BANDS:
        pool = sorted(n for n, s in ok.items() if lo <= s < hi)
        rng.shuffle(pool)
        taken = 0
        for name in pool:
            if taken >= args.per_band:
                break
            if used[story_of(name)] >= MAX_PER_STORY:
                continue
            used[story_of(name)] += 1
            picked.append({"页": name, "连续灰调面积": round(ok[name], 4), "层": f"{lo:.0%}~{hi:.0%}"})
            taken += 1
        print(f"  {lo:.0%}~{hi:.0%}  池 {len(pool):>3} 页 → 取 {taken}")

    picked.sort(key=lambda p: p["连续灰调面积"])
    print(f"\n选中 {len(picked)} 页：\n")
    print(f"{'页':<44}{'连续灰调面积':>12}{'层':>10}")
    for p in picked:
        print(f"{p['页']:<44}{p['连续灰调面积']:>12.1%}{p['层']:>10}")

    json.dump(
        {
            "为什么这么选": (
                "按连续灰调面积分层，四层各三页；排除已判过的四页、非正文页；"
                "同一话最多两页。不按判据读数挑——那等于用判据给自己出题。"
            ),
            "原闸①那四页的面积": {"1004-冒险游戏书-01-1": 0.311, "0969-列车粉笔-01": 0.348,
                                "1000-报仇传票-05-1": 0.486, "1046-梦游盒-04-2": 0.519},
            "选中": picked,
        },
        open(args.out, "w", encoding="utf-8"),
        ensure_ascii=False,
        indent=1,
    )
    print(f"\n→ {args.out}")


if __name__ == "__main__":
    main()
