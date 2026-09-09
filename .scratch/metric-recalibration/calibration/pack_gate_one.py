"""把闸① 补测那 12 页编成真机判读包。

问法与原闸① **完全一致**（`2bit+FS` 对 `4bit 不抖`，整页 1:1）——只有同问法、
同素材、同面板，新的 12 对才能与 AB 交错本第 9~12 对合并成 n=16。

编排照真机包 04 的先例：同一页两版相邻、强制配平、逐对打乱、页上无标注、答案另存。
配平与打乱复用 `pack_device_bundle.arrange`，**那件事只有一处实现**。

用法：

    python pack_gate_one.py <参照页目录> <候选根> <dry.json> <选页 json> <出口目录>
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from pack_device_bundle import arrange, one_pair, pack  # noqa: E402

SEED = 20260910


def main() -> int:
    pages_dir, cand_root, dry_path, picked_path, out = (Path(a) for a in sys.argv[1:6])
    out.mkdir(parents=True, exist_ok=True)

    picked = {p["页"]: p for p in json.load(open(picked_path, encoding="utf-8"))["选中"]}
    dry = {r["页"]: r["读数"] for r in json.load(open(dry_path, encoding="utf-8"))}
    where = {
        "4bit不抖": cand_root / "4bit_off" / pages_dir.parent.name / pages_dir.name,
        "2bit+FS": cand_root / "2bit_fs" / pages_dir.parent.name / pages_dir.name,
    }

    pairs = []
    for number, page, swapped in arrange(sorted(picked), SEED):
        meta, reads = picked[page], dry[page]
        pairs.append(
            {
                **one_pair(number, page, swapped, "4bit不抖", "2bit+FS", where),
                "连续灰调面积": meta["连续灰调面积"],
                "层": meta["层"],
                "判据4bit不抖": reads["4bit"],
                "判据2bit+FS": reads["2bit+FS"],
                "判据说": "4bit不抖" if reads["4bit"] < reads["2bit+FS"] else "2bit+FS",
            }
        )

    pack(out, "G_闸1补测_2bitFS对4bit不抖", pairs)
    (out / "答案（判完再看）.json").write_text(
        json.dumps(
            {
                "G_闸1补测": {
                    "问": "这两版分得出来吗？分得出的话，哪一版更干净？",
                    "与原闸①的关系": (
                        "同问法、同素材（哆啦A梦 v15）、同面板，因此这 12 对可以与 AB 交错本"
                        "第 9~12 对合并成 n=16。原四页不在这一包里，不重判。"
                    ),
                    "怎么选的页": json.load(open(picked_path, encoding="utf-8"))["为什么这么选"],
                    "原闸①那四页": json.load(open(picked_path, encoding="utf-8"))["原闸①那四页的面积"],
                    "对照": [
                        {k: v for k, v in p.items() if not isinstance(v, Path)} for p in pairs
                    ],
                }
            },
            ensure_ascii=False,
            indent=1,
        ),
        encoding="utf-8",
    )
    (out / "怎么看.md").write_text(GUIDE, encoding="utf-8")
    print(f"\n答案键与《怎么看》→ {out}")
    return 0


GUIDE = """# 闸① 补测：十二对

**问：这两版分得出来吗？分得出的话，哪一版更干净？**

十二页取自**哆啦A梦 v15**（一手 8K 源）的连续灰调页，按**连续灰调面积**分层挑
（31%~63%，四层各三页），排除了已经判过的那四页。比的是 `2bit+FS` 对 `4bit 不抖`
——与 AB 交错本第 9~12 对**同一个问法**，所以这一轮判完，那一组的 n 从 4 提到 16。

## 为什么要补这一组

原来那四组真机结论里，**闸① 是样本最薄的一条**（n=4，其中一对反向），
而 `03` 号票的落地记录自己写着「方向清楚但样本薄，**不作定量依据**」。
偏偏它现在是卡住整个判据标定的那一端——拿一个不作定量依据的东西当硬约束，
本身就不一致。**这一组要把它变成作得了数的。**

## 怎么看

1. 拷进 Kobo Libra 2，**关掉阅读器的缩放与白边裁切**，**不要放大**。与前几轮同一条口径。
2. 每一对只回答：**两版分得出来吗？分得出的话，哪一版更干净？**

## 怎么答

`平`（分不出）／`1`（前一张更干净）／`2`（后一张更干净），十二个答案一行：

```
G: 1 2 平 1 2 2 平 1 2 1 平 2
```

**`平` 照旧是正经答案，不要硬选。**连续灰调上这两档本来就接近，
上一轮四对里就有判读者说不好分的；分不出就答 `平`。

## 判完之后

答案与原来那四对合并成 n=16。**结果可能推翻原来那个 3:1 的方向**——那也是结果：
n=4 本来就不够，补测的意义正在于此。真推翻了，`04` 的结论跟着重算，
实验台是现成的（`.scratch/metric-recalibration/calibration/`）。
"""


if __name__ == "__main__":
    sys.exit(main())
