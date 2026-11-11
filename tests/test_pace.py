"""Reading-speed pacing: words per minute and line delays."""
import pytest

from fcc.masks.base import Line
from fcc.pace import (
    DEFAULT_WPM,
    MAX_WPM,
    MIN_DELAY,
    MIN_WPM,
    clamp_wpm,
    line_delay,
    step_wpm,
    words_in,
)


def test_line_delay_follows_words_and_rate():
    assert line_delay(10, 200) == pytest.approx(3.0)
    assert line_delay(10, 400) == pytest.approx(1.5)
    assert line_delay(0, 200) == MIN_DELAY  # filler-only or empty lines never stall or spin


def test_clamp_and_step():
    assert clamp_wpm(10) == MIN_WPM and clamp_wpm(10_000) == MAX_WPM
    assert clamp_wpm("x") == DEFAULT_WPM and clamp_wpm(float("nan")) == DEFAULT_WPM
    assert step_wpm(200, 1) == 220 and step_wpm(220, -1) == 200
    assert step_wpm(MAX_WPM, 1) == MAX_WPM and step_wpm(MIN_WPM, -1) == MIN_WPM


def test_words_in_counts_only_text_segments():
    lines = [
        Line((("   1 ", "dim"), ("iki kelime", "book #fff")), True),
        Line((("noise noise noise", ""),), False),
        Line((("prefix ", ""), ("üç kelime var", "book")), True),
    ]
    assert words_in(lines) == 5

