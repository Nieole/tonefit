"""把标定材料编成真机判读包：交错编排的 cbz ＋ 答案键。

编排照第六轮的先例（`_samples/_实验-判据重标定/真机包/`）：

- **同一页的两个版本相邻**（`01-1` 紧挨 `01-2`），翻一页就是一次 A/B 切换，
  间隔压到 e-ink 的一次刷新。跨文件切换要几秒到几十秒，视觉工作记忆撑不住。
- **顺序强制配平**，不是随机——一半「不抖在前」、一半「FS 在前」。随机洗牌撞成
  同序的概率不低，而判读者一旦察觉「第一张总是某一档」，这一组就废了。
- **页上无标注**，答案另存一份，判完再看。

用法：

    python pack_device_bundle.py <包目录>
"""

import json
import random
import sys
import zipfile
from pathlib import Path

# 编排用的随机种子。定死是为了让同一批材料每次编出来都一样——判读答案按对号回填，
# 编排一变，之前的答案就对不上了。
SEED = 20260909


def arrange(items: list, seed: int) -> list:
    """打乱顺序并**强制配平**：返回 `[(对号, item, 后一张是不是「甲」)]`。

    一半「甲在前」、一半「乙在前」。随机洗牌撞成同序的概率不低（四对全同序是八分之一），
    而判读者一旦察觉「第一张总是某一档」，这一组就废了。

    **配平只在这里做一次。**造材料那几支脚本不管编排——它们只负责造图，
    编排是打包的事。第五轮整批作废就栽在编排上（`judgements/README.md` 的《第五轮》），
    这件事有三份实现是最不该有的。
    """
    order = list(range(len(items)))
    random.Random(seed).shuffle(order)
    return [(f"{rank + 1:02d}", items[i], rank % 2 == 1) for rank, i in enumerate(order)]


def sides(swapped: bool, first: str, second: str) -> tuple:
    """一对的两侧：`swapped` 为真就把两档对调。"""
    return (second, first) if swapped else (first, second)


def pack(out: Path, name: str, pairs: list) -> None:
    """把若干对页编成一本交错的 cbz。

    `pairs` 的每一项：`{"标识": str, "前一张": Path, "后一张": Path, ...}`，
    额外的键原样进答案键。
    """
    cbz = out / f"{name}.cbz"
    with zipfile.ZipFile(cbz, "w", zipfile.ZIP_STORED) as z:
        for i, pair in enumerate(pairs, 1):
            z.write(pair["前一张"], f"{i:02d}-1.png")
            z.write(pair["后一张"], f"{i:02d}-2.png")
    size = cbz.stat().st_size / 1024 / 1024
    print(f"  {cbz.name:<44} {len(pairs):>2} 对 · {size:5.1f} MB")


def one_pair(number: str, page: str, swapped: bool, first: str, second: str, where: dict) -> dict:
    """一对的公共形状：两侧各是哪一档、图从哪来，外加答案键要带的那几项。"""
    left, right = sides(swapped, first, second)
    return {
        "对": number,
        "页": page,
        "前一张": where[left] / page,
        "后一张": where[right] / page,
        "左(-1)": left,
        "右(-2)": right,
    }


def ladder_pairs(ladder_dir: Path, seed: int) -> list:
    """平坦调阶梯：十五格，每格一对（`2bit+FS` 对 `2bit 不抖`）。"""
    manifest = json.load(open(ladder_dir / "阶梯.json", encoding="utf-8"))
    readings = json.load(open(ladder_dir / "读数.json", encoding="utf-8"))
    where = {
        "2bit不抖": ladder_dir / "cand_2bit_off" / "页",
        "2bit+FS": ladder_dir / "cand_2bit_fs" / "页",
    }
    pairs = []
    for number, cell, swapped in arrange(manifest["格"], seed):
        read = readings[cell["页"]]["读数"]
        pairs.append(
            {
                **one_pair(number, cell["页"], swapped, "2bit不抖", "2bit+FS", where),
                "格": cell["格"],
                "离格量": cell["离格量"],
                "背景": cell["背景"],
                "目标灰度": cell["目标灰度"],
                "FS要撒的点": cell["FS在这块上要撒的点"],
                "FS高频起伏": cell["FS高频起伏(sqrt(u(s-u)))"],
                "判据2bit不抖": read["2bit"],
                "判据2bit+FS": read["2bit+FS"],
                "判据说": "不抖更好" if read["2bit"] < read["2bit+FS"] else "FS 更好",
            }
        )
    return pairs


def real_page_pairs(pages: list, where: dict, first: str, second: str, seed: int) -> list:
    """真实整页：一页一对。"""
    return [
        one_pair(number, page.name, swapped, first, second, where)
        for number, page, swapped in arrange(pages, seed)
    ]


def main(out: Path) -> int:
    sp = out.parent
    out.mkdir(parents=True, exist_ok=True)
    answers = {}

    print("编成 cbz：")

    ladder = ladder_pairs(sp / "ladder", SEED)
    pack(out, "L_平坦调阶梯_2bitFS对2bit不抖", ladder)
    answers["L_平坦调阶梯"] = {
        "问": "这一格上，抖动的那一版看得出比不抖动那一版脏吗？",
        "源": json.load(open(sp / "ladder" / "阶梯.json", encoding="utf-8"))["源"],
        "口径": json.load(open(sp / "ladder" / "阶梯.json", encoding="utf-8"))["口径"],
        "对照": [{k: v for k, v in p.items() if not isinstance(v, Path)} for p in ladder],
    }

    ns = real_page_pairs(
        sorted((sp / "ref8_ns23").glob("*.png")),
        {
            "4bit不抖": sp / "ns23_cand" / "4bit_off" / "ref8_ns23",
            "2bit+FS": sp / "ns23_cand" / "2bit_fs" / "ref8_ns23",
        },
        "4bit不抖",
        "2bit+FS",
        seed=SEED,
    )
    pack(out, "M_离格量1的真实页_2bitFS对4bit不抖", ns)
    answers["M_离格量1的真实页"] = {
        "问": "判据说「这页 2bit+FS 不够」的离格量 1 的页，真机上真的不够吗？",
        "源": "N和S 第 23 话（一手 B 类发布版）· 纸白 254 · 离格量 1 · 全 24 页皆同",
        "为什么补这一组": (
            "C 组那八页纸白全是 255（离格量 0），而 N和S 有一半的话纸白是 254。"
            "离格量 1 时 FS 在白底上撒 1/85 = 1.2%，是 B 组那四页的一半，票面记着「没测过」。"
        ),
        "对照": [{k: v for k, v in p.items() if not isinstance(v, Path)} for p in ns],
    }

    anchor_dir = sp / "anchor"
    anchor_meta = json.load(open(anchor_dir / "锚点.json", encoding="utf-8"))
    anchor_reads = json.load(open(anchor_dir / "读数.json", encoding="utf-8"))
    where = {
        "2bit不抖": anchor_dir / "cand_2bit_off" / "页",
        "2bit+FS": anchor_dir / "cand_2bit_fs" / "页",
    }
    anchors = []
    for number, block, swapped in arrange(anchor_meta["块"], SEED):
        read = anchor_reads[block["页"]]["读数"]
        anchors.append(
            {
                **one_pair(number, block["页"], swapped, "2bit不抖", "2bit+FS", where),
                "源页": block["源页"],
                "块均值离格量": block["离格量"],
                "逐像素离格量中位": block["逐像素离格量中位"],
                "判据2bit不抖": read["2bit"],
                "判据2bit+FS": read["2bit+FS"],
                "判据说": "不抖更好" if read["2bit"] < read["2bit+FS"] else "FS 更好",
            }
        )
    pack(out, "N_生态锚点_真实渐变_2bitFS对2bit不抖", anchors)
    answers["N_生态锚点"] = {
        "问": "真实的连续灰调上，抖动的那一版看得出比不抖动那一版脏吗？",
        "是什么": anchor_meta["是什么"],
        "源": anchor_meta["源"],
        "对照": [{k: v for k, v in p.items() if not isinstance(v, Path)} for p in anchors],
    }

    (out / "答案（判完再看）.json").write_text(
        json.dumps(answers, ensure_ascii=False, indent=1), encoding="utf-8"
    )
    # 判读说明是手写的，跟着脚本一起走——包里少了它，判读者就不知道
    # 「关掉缩放与白边裁切、不放大」这两条，而那两条一破整包就白判了。
    (out / "怎么看.md").write_text(
        (Path(__file__).parent / "怎么看.md").read_text(encoding="utf-8"), encoding="utf-8"
    )
    print(f"\n答案键与《怎么看》→ {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1])))
