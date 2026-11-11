from __future__ import annotations

from random import Random

from fcc.masks.base import Line, Mask


def noise_lines(mask: Mask, level: int, rng: Random, width: int) -> list[Line]:
    if not 0 <= level <= 5:
        raise ValueError(f"noise level must be 0..5, got {level}")
    return [mask.noise(rng, width) for _ in range(rng.randint(0, 2 * level))]
