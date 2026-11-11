from __future__ import annotations

from random import Random

from fcc.masks.base import Line, Mask


def noise_count(level: int, rng: Random) -> int:
    if not 0 <= level <= 5:
        raise ValueError(f"noise level must be 0..5, got {level}")
    return rng.randint(0, 2 * level)


def noise_lines(
    mask: Mask, level: int, rng: Random, width: int, count: int | None = None
) -> list[Line]:
    if count is None:
        count = noise_count(level, rng)
    # masks whose filler only makes sense in whole units build the block themselves
    block = getattr(mask, "noise_block", None)
    if block is not None:
        return block(count, rng, width)
    return [mask.noise(rng, width) for _ in range(count)]
