"""造复判包：把 L 组那两格拿回去再判一次，顺带测判读的稳定性。

`try_luminance.py` 试下来，加一维亮度能把 L 组从 11/15 提到 13/15，卡住的是两格：

- `u02_偏暗`（灰度 87）——夹在 `u01_偏暗`（平）与 `u08_偏暗`（平）中间，却给了明确的 FS
- `u42_中灰`（灰度 128）——与同列的 `u21_中灰`（FS）方向相反

**两格都可能落在判读的边界上**，而边界上的答案是随机的、不该拿去定形状。
但这只能问判读者自己，推不出来。

## 怎么问

**同一格放两次，左右顺序相反，位置分开。**判读者不知道哪两对是同一格
（页上无标注、顺序打乱），于是：

- 两次答案**一致** → 那一格是真信号，形状要解释它；
- 两次答案**相反** → 那一格在边界上，13/15 就是这套数据的上限。

外加四格**锚**：上次答得明确的三格（两 FS 一不抖）与一格「平」。锚是用来看
**这一趟判读整体稳不稳**的——锚要是也变了，那就不是这两格的问题，是整批判读的重复性。

用法：

    python pack_recheck.py <ladder 目录> <出口目录>
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from pack_device_bundle import pack  # noqa: E402

SEED = 20260910

# 要复判的两格，各放两次（`次` 记第几次，左右顺序由 `FS在前` 定，两次相反）
RECHECK = [
    {"格": "u02_偏暗", "次": 1, "FS在前": False},
    {"格": "u02_偏暗", "次": 2, "FS在前": True},
    {"格": "u42_中灰", "次": 1, "FS在前": True},
    {"格": "u42_中灰", "次": 2, "FS在前": False},
]

# 锚：上次答得明确的几格，用来看这一趟判读整体稳不稳
ANCHORS = [
    {"格": "u21_中灰", "上次": "2bit+FS", "FS在前": False},
    {"格": "u42_偏暗", "上次": "2bit+FS", "FS在前": True},
    {"格": "u01_近白", "上次": "2bit不抖", "FS在前": True},
    {"格": "u08_偏暗", "上次": "平", "FS在前": False},
]

# 摆位：0~7 共八个位置。两格的两次各自隔开，不让判读者认出「刚才那一张」。
LAYOUT = ["u02_偏暗#1", "u21_中灰", "u42_中灰#1", "u01_近白",
          "u02_偏暗#2", "u08_偏暗", "u42_中灰#2", "u42_偏暗"]


def main() -> int:
    ladder_dir, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    manifest = {g["格"]: g for g in json.load(open(ladder_dir / "阶梯.json", encoding="utf-8"))["格"]}
    reads = json.load(open(ladder_dir / "读数.json", encoding="utf-8"))
    where = {
        "2bit不抖": ladder_dir / "cand_2bit_off" / "页",
        "2bit+FS": ladder_dir / "cand_2bit_fs" / "页",
    }
    plan = {f"{r['格']}#{r['次']}": r for r in RECHECK} | {a["格"]: a for a in ANCHORS}

    pairs = []
    for rank, slot in enumerate(LAYOUT, 1):
        spec = plan[slot]
        cell = manifest[spec["格"]]
        first, second = (
            ("2bit+FS", "2bit不抖") if spec["FS在前"] else ("2bit不抖", "2bit+FS")
        )
        read = reads[cell["页"]]["读数"]
        pairs.append(
            {
                "对": f"{rank:02d}",
                "页": cell["页"],
                "前一张": where[first] / cell["页"],
                "后一张": where[second] / cell["页"],
                "左(-1)": first,
                "右(-2)": second,
                "格": spec["格"],
                "这一格是": "复判" if "次" in spec else "锚",
                "同一格的第几次": spec.get("次"),
                "上次判读者选的": spec.get("上次"),
                "离格量": cell["离格量"],
                "背景": cell["背景"],
                "目标灰度": cell["目标灰度"],
                "FS要撒的点": cell["FS在这块上要撒的点"],
                "判据2bit不抖": read["2bit"],
                "判据2bit+FS": read["2bit+FS"],
            }
        )

    pack(out, "R_复判_2bitFS对2bit不抖", pairs)
    (out / "答案（判完再看）.json").write_text(
        json.dumps(
            {
                "R_复判": {
                    "问": "这一格上，抖动的那一版看得出比不抖动那一版脏吗？",
                    "编排": (
                        "八对里有两格各放了两次（左右顺序相反、位置分开），四格是锚。"
                        "两次答案一致说明那一格是真信号；相反说明它在判读边界上。"
                    ),
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


GUIDE = """# 复判包：八对

与真机包 04 同一个问法、同一批材料，**只是重新编排过**。八对里有两格是上一轮
卡住的，其余是锚。哪一对是哪一格，页上没有标注——照旧判完再对答案。

## 怎么看

1. 拷进 Kobo Libra 2，**关掉阅读器的缩放与白边裁切**，**不要放大**。与上一轮同一条口径。
2. 每一对只回答：**两版分得出来吗？分得出的话，哪一版更干净？**

## 怎么答

`平`（分不出）／`1`（前一张更干净）／`2`（后一张更干净），八个答案一行：

```
R: 1 2 平 1 2 平 1 2
```

**`平` 在这一包里格外要紧。**这一趟要问的正是「有几格其实是分不出的」——
上一轮有两格给了明确答案，但它们与相邻格的模式反着，怀疑当时是在边界上勉强选了一个。
**分不出就答 `平`，不要为了给个结论而硬选。**

## 一句提醒

八对里**有两格各出现了两次**（左右顺序相反、位置隔开）。你不必去找它们，
也不必回想上一轮答了什么——**按当下看到的答就好**。两次答得一样还是不一样，
本身就是这一包要测的东西。
"""


if __name__ == "__main__":
    sys.exit(main())
