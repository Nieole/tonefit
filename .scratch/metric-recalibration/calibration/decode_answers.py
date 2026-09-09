"""把真机判读的三行答案解码成逐对结论，并读出 L 组的翻转点。

判读者答的是**页码**（`1` 前一张更干净 / `2` 后一张 / `平` 分不出），页上无标注，
哪一张是哪一档只有答案键知道。这一支把两者对起来。

L 组的翻转点直接定颗粒地板落在哪一段（对照表见 `ladder_calibrates_floor.py`）。

用法：

    python decode_answers.py <真机包目录> "L: ..." "M: ..." "N: ..."

答案行的格式就是判读者交回来的那一行，组名开头，其余用空格分开。
"""

import json
import sys
from pathlib import Path

CLEANER = {"1": "前", "2": "后", "平": "平"}


def parse(line: str):
    """`L 1 2 平 ...` → (组名首字母, [答案...])。"""
    tokens = line.replace(":", " ").replace("：", " ").split()
    group = tokens[0]
    answers = [t for t in tokens[1:] if t in CLEANER]
    return group, answers


def decode(bundle: Path, lines: list) -> dict:
    answers = json.load(open(bundle / "答案（判完再看）.json", encoding="utf-8"))
    by_initial = {name[0]: name for name in answers}
    out = {}
    for line in lines:
        initial, given = parse(line)
        name = by_initial[initial]
        pairs = answers[name]["对照"]
        assert len(given) == len(pairs), f"{name}：给了 {len(given)} 个答案，有 {len(pairs)} 对"
        rows = []
        for pair, answer in zip(pairs, given):
            if answer == "平":
                cleaner = "平"
            else:
                cleaner = pair["左(-1)"] if answer == "1" else pair["右(-2)"]
            rows.append({**pair, "答": answer, "判读者选的": cleaner})
        out[name] = rows
    return out


def main() -> int:
    bundle = Path(sys.argv[1])
    decoded = decode(bundle, sys.argv[2:])

    ladder = decoded.get("L_平坦调阶梯")
    if ladder:
        print("=== L 组：平坦调阶梯 ===\n")
        print(f"{'离格量':>6}{'背景':>6}{'灰度':>6}{'撒点':>8}{'答':>4}{'判读者选的':>12}"
              f"{'判据说':>10}{'':>4}")
        for row in sorted(ladder, key=lambda r: (r["离格量"], r["目标灰度"]), reverse=True):
            agree = "" if row["判读者选的"] == "平" else (
                "  ✓" if row["判读者选的"].replace("2bit不抖", "不抖更好").replace(
                    "2bit+FS", "FS 更好") == row["判据说"] else "  ✗")
            print(
                f"{row['离格量']:>6}{row['背景']:>6}{row['目标灰度']:>6}"
                f"{row['FS要撒的点']:>8.1%}{row['答']:>4}{row['判读者选的']:>12}"
                f"{row['判据说']:>10}{agree}"
            )

        print("\n按《离格量》归拢——翻转点就在这一列上：\n")
        print(f"{'离格量 u':>8}{'撒点':>8}{'三种亮度上判读者选的':>34}")
        for u in sorted({r["离格量"] for r in ladder}):
            got = [r for r in ladder if r["离格量"] == u]
            picks = " · ".join(f"{r['背景']}:{r['判读者选的']}" for r in got)
            print(f"{u:>8}{got[0]['FS要撒的点']:>8.1%}   {picks}")

    for name in ("M_离格量1的真实页", "N_生态锚点"):
        rows = decoded.get(name)
        if not rows:
            continue
        print(f"\n\n=== {name} ===\n")
        for row in rows:
            extra = ""
            if "块均值离格量" in row:
                extra = f"  （块均值离格 {row['块均值离格量']}）"
            print(
                f"  第 {row['对']} 对  {row.get('页', ''):<28} 答 {row['答']}"
                f" → **{row['判读者选的']}**{extra}"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main())
