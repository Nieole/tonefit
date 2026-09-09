"""造真机材料的公共件：面板尺寸，以及「把一块铺满整页」。

两支造材料的脚本（`build_flat_ladder.py` 与 `build_anchor.py`）都要这一件事，
放一处而不是各写一份。
"""

import numpy as np

# 面板尺寸（kobo-libra-2）。裁块铺满它，判读因此是**整页 1:1**，不是局部放大块
# ——measurements《成品库上判不出阈值的原因是空间尺度》记着那条能力边界。
PANEL = (1264, 1680)


def tile_to_panel(block: np.ndarray) -> np.ndarray:
    """把一块平铺成整页。

    像素全部是真实扫描来的，接缝落在平坦区上、看不出来；而抖动是在**拼好之后**
    才做的，误差扩散因此在整页上连续，与真实页上一样。
    """
    width, height = PANEL
    reps = (height // block.shape[0] + 1, width // block.shape[1] + 1)
    return np.tile(block, reps)[:height, :width]
