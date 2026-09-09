"""第十轮真机包：八对，两本，答两个今天没人答得了的问题。

**问题一（闸①，四对）——`4bit+FS` 到底买不买得到东西？**

第六轮 A 组判过 `4bit+FS` 对 `4bit 不抖`，8 对全答「平」，`metric-recalibration/06`
（候选集删掉 `4bit+FS`）全部依据就是它。但那八页的**色带风险面积只有 8.71~18.78%**
（纸白全 255、离格全 0、FS 在白底一个点都不撒），而闸① 那 12 页是 **33.99~52.20%**，
两侧零重叠、间隔 15 个点——**A 组测不到 `4bit+FS` 唯一的用处**。

整卷 `--dry-run` 实测：闸① 12 页今天 10 页判 `4bit+FS`、2 页判 `4bit 不抖`；
删掉 `4bit+FS` 之后 **12/12 落到 `4bit 不抖`**，而真机第九轮 12/12 说那一档更差。

**问题二（对齐后 B 组，四对）——纸白对齐之后，那四页该判哪一档？**

`tone-alignment/05` 的落地前置。B 组真机 4/4 判 `4bit 不抖` 更干净，**那是在未对齐的
素材上判的**；对齐后纸白钳到 255、FS 在白底一个点都不撒，「不可接受」的原因当场消失。
对齐后判据读数 `2bit+FS` 1.699~1.840（与票面记的 1.70~1.84 逐位吻合），判定翻成 `2bit+FS`。
**「对齐后该判哪一档」今天没有任何真机数据。**

## 材料怎么来的

闸① 四页取自 `_samples/_实验-判据重标定/闸1补测包/参照8bit/G_闸1补测`（第九轮那 12 页的参照），
候选由 `tonefit --no-crop --no-split` 渲，**不用脚本自己量化**（少一层等价性风险）。

对齐后那四页：`--white-align-limit` 眼下**还没落地**（`tone-alignment/01` 是 `ready-for-agent`），
所以对齐这一步照 `01` 的定义在这里复现——纸白量法直接调 `survey_flat.paper_white`（同一份实现），
钳制是 `[纸白,255] → 255`。**复现验过**：四页量出纸白 253、离格 2，与第六轮 JSON 记的
`源纸白.纸白 = 253`、`到2bit格点 = 2` 逐字相同；钳后纸白 255、离格 0。

> ⚠️ **这四页的判读结论要带一条限定**：材料出自对齐的**复现**，不是产品。
> `01` 落地之后要用产品重渲一遍比字节；对不上就得重判。

## 编排

同一页两版相邻、页上无标注、答案另存——照第六轮与真机包 04 的先例。

**配平这一处与 `pack_device_bundle.arrange` 不同，是有意的**：那一支按 `rank % 2 == 1`
严格左右交替，于是「位置偏好已排除」成了循环论证（判读者只要察觉隔一对换一次就废了）。
这里改成**打乱一份配平好的翻转表**：数目仍然一半一半，而哪一对翻不可预测。

用法：

    python pack_round_ten.py <出口目录>
"""

import json
import random
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from pack_device_bundle import pack, sides  # noqa: E402

SEED = 20260911

WORK = Path(
    "C:/Users/saber/AppData/Local/Temp/claude/"
    "C--Users-saber-RustProjects-tonefit/ca9b00fc-1bfb-4d48-a359-7bc1bed55ad9/scratchpad/round10"
)

# 闸① 四页：两页问 `4bit+FS` 对 `2bit+FS`（今天产品给的那一档够不够好），
# 两页问 `4bit+FS` 对 `4bit 不抖`（把 A 组原样搬到连续灰调页上，直接结掉 `mr/06`）。
# 四页互不重复——同一页出现两次会让判读者锚在自己上一次的答案上。
GATE_ONE = [
    ("FCD-DFC-15-1030-洗脑喇叭-04-2", "4bit+FS", "2bit+FS", 0.3500),
    ("FCD-DFC-15-1010-障眼法涂鸦笔-03-1", "4bit+FS", "2bit+FS", 0.5130),
    ("FCD-DFC-15-1054-梦境导演椅-05-1", "4bit+FS", "4bit不抖", 0.5220),
    ("FCD-DFC-15-1056-随心所欲照片列印机-05-1", "4bit+FS", "4bit不抖", 0.4489),
]

ALIGNED = ["MHZ01_088-2", "MHZ01_089-1", "MHZ01_089-2", "MHZ01_090"]

WHERE_G1 = {
    "4bit+FS": WORK / "cand/4bit_fs/G_闸1补测",
    "2bit+FS": WORK / "cand/2bit_fs/G_闸1补测",
    "4bit不抖": WORK / "cand/4bit_off/G_闸1补测",
}
WHERE_ALIGNED = {
    "2bit+FS": WORK / "cand_aligned/2bit_fs/ref_aligned",
    "4bit不抖": WORK / "cand_aligned/4bit_off/ref_aligned",
}


def balanced_flips(count: int, seed: int) -> list:
    """一半翻、一半不翻，**哪几对翻是打乱的**。

    `pack_device_bundle.arrange` 用的是 `rank % 2 == 1`——数目配平了，但左右**可预测**，
    于是「位置偏好已排除」这句话成了循环论证（停车场记着这一条）。
    这里保留配平、去掉可预测性。
    """
    flips = [i < count // 2 for i in range(count)]
    random.Random(seed).shuffle(flips)
    return flips


def build(items: list, where: dict, seed: int, extra) -> list:
    order = list(range(len(items)))
    random.Random(seed).shuffle(order)
    flips = balanced_flips(len(items), seed + 1)
    pairs = []
    for rank, (i, swapped) in enumerate(zip(order, flips), 1):
        item = items[i]
        page, first, second = item[0], item[1], item[2]
        left, right = sides(swapped, first, second)
        pairs.append(
            {
                "对": f"{rank:02d}",
                "页": f"{page}.png",
                "前一张": where[left] / f"{page}.png",
                "后一张": where[right] / f"{page}.png",
                "左(-1)": left,
                "右(-2)": right,
                **extra(item),
            }
        )
    return pairs


def main(out: Path) -> int:
    out.mkdir(parents=True, exist_ok=True)
    reads = json.load(open(WORK / "dry_g1.json", encoding="utf-8"))
    aligned_reads = json.load(open(WORK / "dry_aligned.json", encoding="utf-8"))

    def g1_extra(item):
        page, a, b, area = item
        r = reads[page]["读数"]
        ka = {"4bit+FS": "4bit+FS", "2bit+FS": "2bit+FS", "4bit不抖": "4bit"}
        return {
            "色带风险面积": round(area, 4),
            "今天产品判": reads[page]["判定"],
            f"判据{a}": r[ka[a]],
            f"判据{b}": r[ka[b]],
            "判据说": a if r[ka[a]] < r[ka[b]] else b,
        }

    def al_extra(item):
        page = item[0]
        r = aligned_reads[page]["读数"]
        return {
            "对齐": "复现（01 未落地）：纸白 253 → 钳 [253,255]→255，钳后纸白 255、离格 0",
            "对齐后产品判": aligned_reads[page]["判定"],
            "判据2bit+FS": r["2bit+FS"],
            "判据4bit不抖": r["4bit"],
            "判据说": "2bit+FS" if r["2bit+FS"] < r["4bit"] else "4bit不抖",
            "对齐前真机": "4bit不抖（第六轮 B 组 4/4）",
        }

    print("编成 cbz：")
    g1 = build(GATE_ONE, WHERE_G1, SEED, g1_extra)
    pack(out, "P_闸1_4bitFS买不买得到东西", g1)
    al = build([(p, "2bit+FS", "4bit不抖") for p in ALIGNED], WHERE_ALIGNED, SEED + 7, al_extra)
    pack(out, "W_对齐后B组_2bitFS对4bit不抖", al)

    answers = {
        "轮次": "第十轮：闸① `4bit+FS` 四对 ＋ 对齐后 B 组四对",
        "面板": "kobo-libra-2",
        "口径": {
            "观看": "与第六～九轮同一条：整页 1:1，关缩放与白边裁切，不放大",
            "编排": "同页两版相邻、页上无标注、配平后**打乱**左右（不是隔对交替）",
            "种子": SEED,
        },
        "P_闸1": [{k: (str(v) if isinstance(v, Path) else v) for k, v in p.items()} for p in g1],
        "W_对齐后B组": [{k: (str(v) if isinstance(v, Path) else v) for k, v in p.items()} for p in al],
    }
    (out / "答案（判完再看）.json").write_text(
        json.dumps(answers, ensure_ascii=False, indent=1), encoding="utf-8"
    )
    (out / "怎么看.md").write_text(GUIDE, encoding="utf-8")
    print(f"\n答案键与说明写在 {out}")
    return 0


GUIDE = """# 第十轮：两本，八对

**判之前不要打开 `答案（判完再看）.json`。**

两本各是一问，**问法不同，别混着答**。

## P_闸1_4bitFS买不买得到东西（四对）

连续灰调页。每一对是同一页的两个版本，`01-1` 与 `01-2` 相邻，翻一页就是一次 A/B 切换。

**问：这两版分得出来吗？分得出的话哪一版更干净？**

答成 `1` / `2` / `平`。**分不出就答「平」**，不要勉强挑一个——这一问最要紧的可能答案就是「平」。
分得出来的话，简单说一句**是凭什么分出来的**（色带？颗粒？文字？整体偏色？）。
那句话比答案本身值钱：第九轮全部十二对的理由都是「另外一张有轻微色彩断层」，
而正是那句话把病灶指向了色带。

## W_对齐后B组_2bitFS对4bit不抖（四对）

白底文字线稿（武器種族傳說），**纸白已经对齐到 255**。

这四页你判过一次（第六轮 B 组，4/4 判「4bit 不抖更干净」）——**那一次是在没对齐的素材上判的**，
白底上撒着 2.4% 的抖动点。这一次白底上一个点都没有。

**问：这两版哪一版更干净？**同样，分不出就答「平」。

## 怎么回答

每本按对号报一串就行，例如：

```
P 1 平 2 1
W 2 2 平 1
```

再加上分得出来的那几对是凭什么分的。
"""


if __name__ == "__main__":
    raise SystemExit(main(Path(sys.argv[1])))
