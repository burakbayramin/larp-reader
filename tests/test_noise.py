from random import Random

import pytest

from fcc.masks.code import CodeMask
from fcc.noise import noise_lines

m = CodeMask("python")


def test_level_zero_gives_no_noise():
    assert noise_lines(m, 0, Random(1), 80) == []


def test_level_bounds():
    for lvl in range(6):
        counts = [len(noise_lines(m, lvl, Random(seed), 80)) for seed in range(50)]
        assert max(counts) <= 2 * lvl
    # high level actually produces noise, and never book lines
    lines = [ln for seed in range(50) for ln in noise_lines(m, 5, Random(seed), 80)]
    assert lines and not any(ln.is_book for ln in lines)
    with pytest.raises(ValueError):
        noise_lines(m, 6, Random(1), 80)
    with pytest.raises(ValueError):
        noise_lines(m, -1, Random(1), 80)


def test_count_varies_and_reaches_max():
    counts = {len(noise_lines(m, 2, Random(s), 80)) for s in range(200)}
    assert counts == {0, 1, 2, 3, 4}
