from fcc.book import Book, Chapter, Paragraph
from fcc.masks.code import CodeMask
from fcc.session import Session

TEXT = "Bu bir deneme paragrafidir ve oldukca uzun bir cumle icerir. " * 6


def make_book(n=3):
    return Book("bk", (Chapter("1", tuple(Paragraph(TEXT + str(i)) for i in range(n))),))


def mk(book=None, noise=3, width=80):
    return Session(book or make_book(), CodeMask("python"), noise=noise, width=width)


def test_same_paragraph_renders_identically_after_back_and_forth():
    s = mk()
    first = s.paragraph_lines((0, 0))
    assert first
    assert s.advance() is True
    assert s.back() is True
    assert s.paragraph_lines((0, 0)) == first
    assert s.pos == (0, 0)


def test_paragraph_lines_contains_book_lines_and_noise():
    s = mk(noise=5)
    lines = s.paragraph_lines((0, 1))
    assert any(ln.is_book for ln in lines)
    assert any(not ln.is_book for ln in lines)


def test_noise_zero_has_only_book_lines():
    lines = mk(noise=0).paragraph_lines((0, 0))
    assert any(ln.is_book for ln in lines)
    # the mask's own scaffolding is deterministic, so noise=0 adds nothing on top of render
    from random import Random
    s = mk(noise=0)
    expected = s.mask.render(s.book.paragraph((0, 0)), (0, 0), 80, Random("bk:(0, 0):code:0"), 1)
    assert lines == expected


def test_line_numbers_start_from_index_times_40():
    s = mk(noise=0)
    mask = CodeMask("python")
    from random import Random
    expected = mask.render(
        s.book.paragraph((0, 2)), (0, 2), 80, Random("bk:(0, 2):code:0"), 2 * 40 + 1
    )
    assert s.paragraph_lines((0, 2)) == expected


def test_progress_counts_paragraphs_not_noise():
    s = mk(noise=5)
    assert s.progress() == (1, 3)
    s.advance(2)
    assert s.progress() == (3, 3)


def test_lines_from_fills_requested_book_lines():
    s = mk()
    lines = s.lines_from((0, 0), 8)
    assert sum(ln.is_book for ln in lines) >= 8
    first_para = s.paragraph_lines((0, 0))
    assert lines[: len(first_para)] == first_para  # whole paragraphs, joined


def test_lines_from_stops_at_book_end():
    s = mk()
    lines = s.lines_from((0, 2), 10_000)
    assert lines == s.paragraph_lines((0, 2))


def test_advance_stops_at_end_returns_false():
    s = mk()
    assert s.advance(2) is True and s.pos == (0, 2)
    assert s.advance() is False and s.pos == (0, 2)
    assert s.advance(5) is False


def test_back_stops_at_start_returns_false():
    s = mk()
    assert s.back() is False and s.pos == (0, 0)
    s.advance(2)
    assert s.back(2) is True and s.pos == (0, 0)


def test_setters_change_rendering():
    s = mk(noise=0)
    a = s.paragraph_lines((0, 0))
    s.width = 40
    assert s.width == 40 and s.paragraph_lines((0, 0)) != a
    s.mask = CodeMask("python")
    s.noise = 4
    assert s.noise == 4 and s.mask.name == "code"
