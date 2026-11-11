"""Reading-speed pacing: words per minute, line delays and learning from manual paging."""
import pytest

from fcc.masks.base import Line
from fcc.pace import (
    DEFAULT_WPM,
    MAX_WPM,
    MIN_DELAY,
    MIN_WPM,
    clamp_wpm,
    learn,
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


def test_learn_moves_toward_the_measured_rate():
    # 100 words in 20 s = 300 wpm; a quarter of the way from 200
    assert learn(200, 100, 20.0) == 225


@pytest.mark.parametrize(
    "words, seconds",
    [(100, 1.0), (100, 600.0), (5, 10.0)],  # skimmed, walked away, too little text
)
def test_learn_ignores_unreliable_screens(words, seconds):
    assert learn(200, words, seconds) is None


def test_learn_result_is_clamped():
    assert learn(MAX_WPM, 2000, 2.5) == MAX_WPM
