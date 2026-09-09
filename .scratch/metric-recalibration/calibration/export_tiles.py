"""把标定用的那几页逐块的四个原始量导出成一份 npz，并当场做口径自检。

**自检是这一步的全部意义**：用今天在 `src/metric.rs` 里的那五个常数重算，
读数必须与 `tonefit --dry-run` 的那一列对得上。对不上就先查口径。
（judgements/README 的《重算》记着同一条要求。）

用法：

    python export_tiles.py <ref8 目录> <cand 目录> <dry.json> <出口 npz>

参照根目录下一个子目录是一组，组里是参照 8bit 的页；候选在
`<cand>/<档名>/<参照根目录名>/<组>/<页>`
是 tonefit 渲出的候选。候选**不由本脚本量化**——直接用 tonefit 的产物，
少一层「Python 量化与 quantize.rs 等不等价」的风险。
"""

import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from metric_replica import (  # noqa: E402
    LIVE,
    candidate_side,
    page_score,
    reference_side,
)

PPI = 300  # kobo-libra-2
DEPTHS = {
    "1bit": (1, "1bit_off"),
    "1bit+FS": (1, "1bit_fs"),
    "2bit": (2, "2bit_off"),
    "2bit+FS": (2, "2bit_fs"),
    "4bit": (4, "4bit_off"),
    "4bit+FS": (4, "4bit_fs"),
}


def load(path: Path) -> np.ndarray:
    return np.asarray(Image.open(path).convert("L"), dtype=np.uint8)


def main(ref_root: Path, cand_root: Path, dry_path: Path, out: Path) -> int:
    dry = {(r["组"], r["页"]): r for r in json.load(open(dry_path, encoding="utf-8"))}
    store: dict[str, np.ndarray] = {}
    worst = 0.0
    print(f"{'组':<14}{'页':<34}{'档':<9}{'复现':>9}{'dry-run':>9}{'差':>8}")
    for group_dir in sorted(p for p in ref_root.iterdir() if p.is_dir()):
        for page in sorted(group_dir.glob("*.png")):
            key = (group_dir.name, page.name)
            ref_image = load(page)
            ref = reference_side(ref_image, PPI)
            store[f"{key[0]}|{key[1]}|ref_grain"] = ref["grain"]
            store[f"{key[0]}|{key[1]}|activity"] = ref["activity"]
            store[f"{key[0]}|{key[1]}|tone"] = ref["tone"]
            store[f"{key[0]}|{key[1]}|flat_activity"] = ref["flat_activity"]
            for label, (depth, folder) in DEPTHS.items():
                # tonefit 把输出镜像到 `<out>/<源目录名>/…`，所以候选那一层的目录名
                # 就是参照根目录自己的名字——不写死 "ref8"，换一批素材才不用改代码。
                cand_path = cand_root / folder / ref_root.name / group_dir.name / page.name
                cand = candidate_side(load(cand_path), ref)
                store[f"{key[0]}|{key[1]}|{label}|low_pass_error"] = cand["low_pass_error"]
                store[f"{key[0]}|{key[1]}|{label}|grain"] = cand["grain"]
                store[f"{key[0]}|{key[1]}|{label}|depth"] = np.asarray([depth])

                mine = page_score(ref, cand, depth, **LIVE)
                theirs = dry[key]["读数"][label]
                diff = abs(mine - theirs)
                worst = max(worst, diff)
                flag = "" if diff < 0.01 else "  ← 对不上"
                print(
                    f"{key[0]:<14}{key[1]:<34}{label:<9}"
                    f"{mine:9.3f}{theirs:9.3f}{diff:8.4f}{flag}"
                )

    np.savez_compressed(out, **store)
    print(f"\n最大偏差 {worst:.4f}（判据读数印到小数点后三位）")
    print(f"逐块四原始量已存 → {out}")
    return 0 if worst < 0.01 else 1


if __name__ == "__main__":
    sys.exit(main(*(Path(a) for a in sys.argv[1:5])))
