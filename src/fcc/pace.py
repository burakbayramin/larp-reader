"""Reading speed in words per minute: auto-scroll delays and learning from manual paging."""
from __future__ import annotations

import math

from fcc.masks.base import Line

DEFAULT_WPM = 220
MIN_WPM = 60
MAX_WPM = 1500
WPM_STEP = 1.1
MIN_DELAY = 0.12  # seconds; lines without text still take a beat

# a screen counts as a reading sample only within these bounds
MIN_SAMPLE_SECONDS = 2.0  # quicker than this is skimming or paging past
MAX_SAMPLE_SECONDS = 300.0  # longer than this the reader was probably away
MIN_SAMPLE_WORDS = 15
LEARN_WEIGHT = 0.25  # how far one screen moves the estimate toward its own rate


def clamp_wpm(value) -> int:
    """A usable rate from untrusted input; the default when it isn't a number."""
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        return DEFAULT_WPM
    return int(min(MAX_WPM, max(MIN_WPM, round(value))))


def step_wpm(wpm: int, direction: int) -> int:
    """One `+`/`-` step: about 10% faster or slower."""
    factor = WPM_STEP if direction > 0 else 1 / WPM_STEP
    return clamp_wpm(wpm * factor)


def line_delay(words: int, wpm: int) -> float:
    """Seconds to leave a line of `words` words before the next one appears."""
    return max(MIN_DELAY, words * 60 / max(1, wpm))


def words_in(lines: list[Line]) -> int:
    """Words of text (not chrome or filler) in the given lines."""
    count = 0
    for line in lines:
        if not line.is_book:
            continue
        for text, style in line.segments:
            if "book" in style.split():
                count += len(text.split())
    return count


def learn(wpm: int, words: int, seconds: float) -> int | None:
    """The rate after reading `words` words in `seconds`, or None if the screen says nothing reliable."""
    if words < MIN_SAMPLE_WORDS or not MIN_SAMPLE_SECONDS <= seconds <= MAX_SAMPLE_SECONDS:
        return None
    measured = words * 60 / seconds
    return clamp_wpm(wpm + (measured - wpm) * LEARN_WEIGHT)
