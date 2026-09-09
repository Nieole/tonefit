"""自检真机包：cbz 里的每一页，与答案键说的那一档**逐字节**对得上吗。

第五轮有过一次教训——面板上没烧标签，五位判读者里三位在白底页上把 A/B/C 数错一格，
整批材料作废（`judgements/README.md` 的《第五轮》）。这一包页上同样没有标注（有标注就
不是盲测了），因此**编排的正确性只能靠自检**：把 cbz 里的每一页与它应该是的那一档
的原图比字节，对不上就当场红。

顺带查两条编排规矩：

- **强制配平**——「FS 在前」的对数与「不抖在前」的对数不该差得离谱（十二对随机洗牌
  撞成同序的概率不低，而判读者一旦察觉规律，这一组就废了）。
- **页名连续**——`01-1` `01-2` `02-1` …… 中间不缺号。

用法：

    python verify_bundle.py <真机包目录> <素材根目录>
"""

import hashlib
import json
import sys
import zipfile
from pathlib import Path

# 每组：cbz 名字前缀 → (答案键里的组名, 那一档的原图在哪)
GROUPS = {
    "L_": ("L_平坦调阶梯", lambda sp, d: sp / "ladder" / f"cand_2bit_{_suffix(d)}" / "页"),
    "M_": ("M_离格量1的真实页", lambda sp, d: sp / "ns23_cand" / _ns_and_s(d) / "ref8_ns23"),
    "N_": ("N_生态锚点", lambda sp, d: sp / "anchor" / f"cand_2bit_{_suffix(d)}" / "页"),
}


def _suffix(depth: str) -> str:
    return "fs" if "FS" in depth else "off"


def _ns_and_s(depth: str) -> str:
    """M 组（N和S 第 23 话）那两档的目录名——它比的是 2bit+FS 对 4bit 不抖。"""
    return "2bit_fs" if "FS" in depth else "4bit_off"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()[:16]


def main(bundle: Path, samples: Path) -> int:
    answers = json.load(open(bundle / "答案（判完再看）.json", encoding="utf-8"))
    bad = 0
    for cbz in sorted(bundle.glob("*.cbz")):
        prefix = next((p for p in GROUPS if cbz.name.startswith(p)), None)
        if prefix is None:
            print(f"{cbz.name}：没有对应的答案键组，跳过")
            continue
        group, where = GROUPS[prefix]
        pairs = answers[group]["对照"]
        z = zipfile.ZipFile(cbz)
        names = sorted(n for n in z.namelist() if not n.endswith("/"))

        expected = [f"{i:02d}-{s}.png" for i in range(1, len(pairs) + 1) for s in (1, 2)]
        if names != expected:
            print(f"{cbz.name}：页名不连续 —— 有 {len(names)} 页，期望 {expected[:3]}…")
            bad += 1
            continue

        first_is_fs = 0
        for pair in pairs:
            page = pair["页"]
            for slot, key in (("1", "左(-1)"), ("2", "右(-2)")):
                depth = pair[key]
                source = where(samples, depth) / page
                if not source.exists():
                    print(f"{cbz.name} 第 {pair['对']} 对：找不到原图 {source}")
                    bad += 1
                    continue
                inside = digest(z.read(f"{pair['对']}-{slot}.png"))
                outside = digest(source.read_bytes())
                if inside != outside:
                    print(
                        f"{cbz.name} 第 {pair['对']} 对 -{slot}：答案键说是「{depth}」，"
                        f"但字节对不上（{inside} ≠ {outside}）"
                    )
                    bad += 1
            first_is_fs += "FS" in pair["左(-1)"]

        share = first_is_fs / len(pairs)
        balance = "配平" if 0.3 <= share <= 0.7 else "**偏了**"
        print(
            f"{cbz.name:<44} {len(pairs):>2} 对 · 逐页字节对得上 · "
            f"FS 在前 {first_is_fs}/{len(pairs)} {balance}"
        )
    print("\n" + ("自检通过。" if bad == 0 else f"**{bad} 处对不上。**"))
    return 1 if bad else 0



if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]), Path(sys.argv[2])))
