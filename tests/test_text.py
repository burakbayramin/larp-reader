from fcc.text import normalize_paragraphs, wrap


def test_normalize_joins_hyphenated_line_breaks():
    assert normalize_paragraphs(["Raskol-\nnikov uyandı.", "  ", "Oda\ndar."]) == ["Raskolnikov uyandı.", "Oda dar."]


def test_wrap_respects_width_and_keeps_all_words():
    lines = wrap("Oda dar ve basıktı, tavanı alçaktı.", 12)
    assert all(len(l) <= 12 for l in lines)
    assert " ".join(lines).split() == "Oda dar ve basıktı, tavanı alçaktı.".split()


def test_wrap_hard_splits_overlong_word():
    assert wrap("a" * 30, 10) == ["a" * 10] * 3


def test_wrap_nonpositive_width_does_not_hang():
    for w in (0, -5):
        lines = wrap("ab cd", w)
        assert "".join(lines) == "abcd"


# -- ragged lines: text laid out like source code, not a justified block --------

from fcc.text import CODE_LINE_MAX, ragged

PROSE = (
    "Sabah erkenden uyandı, pencereyi açtı ve dışarı baktı. Sokaklar sessizdi; "
    "yalnızca uzaktan bir vapur düdüğü geliyordu. Çayını demledi, masaya oturdu, "
    "dünkü mektubu yeniden okudu. Cevap yazmak istiyordu ama ne diyeceğini bilmiyordu. "
) * 4


def test_ragged_keeps_every_word_in_order_and_fits():
    for width in (20, 60, 120, 200):
        lines = ragged(PROSE, width)
        assert " ".join(lines).split() == PROSE.split()
        assert all(0 < len(l) <= min(width, CODE_LINE_MAX) for l in lines)


def test_ragged_line_ends_vary_like_code():
    lines = ragged(PROSE, 100)[:-1]  # the last line is short anyway
    lengths = {len(l) for l in lines}
    assert len(lengths) > len(lines) // 2  # not a block of equal lines
    assert min(lengths) < 0.7 * max(lengths)


def test_ragged_prefers_breaking_after_punctuation():
    ends = [l[-1] for l in ragged(PROSE, 100)]
    assert sum(e in ".,;:!?" for e in ends) > len(ends) // 2


def test_ragged_is_stable_for_the_same_text_and_width():
    assert ragged(PROSE, 80) == ragged(PROSE, 80)


def test_ragged_hard_splits_overlong_words_and_tiny_widths():
    assert ragged("a" * 25, 10) == ["a" * 10, "a" * 10, "a" * 5]
    assert " ".join(ragged(PROSE, 1)).split() != []  # no hang
