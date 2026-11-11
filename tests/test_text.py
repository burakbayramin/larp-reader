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
