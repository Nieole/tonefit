"""02 号票的验收：把判据读数换成新判据的，重算三个 ρ。

    python recompute.py 第四轮判读.json                # 用文件里的旧读数，应复现 +0.539 / -0.442 / -0.240
    python recompute.py 第四轮判读.json 新判据读数.json  # 新读数：{"争议01": {"2bit+FS": x, "4bit": y}, ...}

判读的那一半（评级）不重做——它问的是图好不好看，与判据无关。
"""
import json, sys
import numpy as np
from scipy.stats import spearmanr

data = json.load(open(sys.argv[1], encoding="utf8"))
fresh = json.load(open(sys.argv[2], encoding="utf8")) if len(sys.argv) > 2 else None

rd2, rd4, g2, g4, a2 = [], [], [], [], []
for p in data["页"]:
    r = fresh[p["id"]] if fresh else p["旧判据读数"]
    rd2.append(r["2bit+FS"]); rd4.append(r["4bit"])
    g2.append(np.mean(p["判读"]["2bit+FS"]["评级"]))
    g4.append(np.mean(p["判读"]["4bit不抖"]["评级"]))
    a2.append(np.mean(p["判读"]["2bit+FS"]["接受"]))
rd2, rd4, g2, g4, a2 = map(np.array, (rd2, rd4, g2, g4, a2))

rows = [("同档排页   读数 vs 评级", rd2, g2, "+"),
        ("同档·接受 读数 vs 接受率", rd2, a2, "-"),
        ("跨档选谁   优劣差 vs 优劣差", rd2 - rd4, g2 - g4, "+")]
print(f"n = {len(rd2)}   来源：{'新读数' if fresh else '文件里的旧读数'}\n")
for name, x, y, want in rows:
    s = spearmanr(x, y)
    ok = (s.statistic > 0) if want == "+" else (s.statistic < 0)
    print(f"  {name:<28} rho = {s.statistic:+.3f}  p = {s.pvalue:.3f}"
          f"   期望 {want}  {'方向对' if ok else '方向反'}"
          f"{'  显著' if s.pvalue < 0.05 else ''}")
