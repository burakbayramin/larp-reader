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
    first = len(s.mask.header(Random("bk"), 80)) + 1
    expected = s.mask.render(s.book.paragraph((0, 0)), (0, 0), 80, Random("bk:(0, 0):code:0"), first)
    assert lines == expected


def gutter_numbers(lines):
    return [int(ln.plain[:4]) for ln in lines]


def test_line_numbers_run_on_from_the_header_without_gaps():
    # like a real file: the header is lines 1..h and every paragraph continues the count
    from random import Random
    for noise in (0, 3):
        s = mk(make_book(6), noise=noise)
        header = s.mask.header(Random("bk"), 80)
        numbers = gutter_numbers(header)
        for i in range(6):
            numbers += gutter_numbers(s.paragraph_lines((0, i)))
        assert numbers == list(range(1, len(numbers) + 1))


def test_line_numbers_do_not_depend_on_visiting_order():
    a, b = mk(make_book(6)), mk(make_book(6))
    forward = [a.paragraph_lines((0, i)) for i in range(6)]
    backward = [b.paragraph_lines((0, i)) for i in reversed(range(6))]
    assert forward == backward[::-1]


def test_line_numbers_follow_a_width_or_noise_change():
    s = mk(make_book(4))
    wide = gutter_numbers(s.paragraph_lines((0, 3)))[0]
    s.width = 40
    narrow = gutter_numbers(s.paragraph_lines((0, 3)))[0]
    assert narrow > wide  # narrower pane, more wrapped lines above
    fresh = mk(make_book(4), width=40)
    assert s.paragraph_lines((0, 3)) == fresh.paragraph_lines((0, 3))


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


def test_window_spans_paragraphs_from_line_offset():
    s = mk(noise=2)
    stream = s.paragraph_lines((0, 0)) + s.paragraph_lines((0, 1)) + s.paragraph_lines((0, 2))
    assert s.window(10_000) == stream
    s.line_offset = 3
    assert s.window(5) == stream[3:8]


def test_scroll_crosses_paragraphs_both_ways_and_clamps():
    s = mk(noise=2)
    first = len(s.paragraph_lines((0, 0)))
    stream = s.window(10_000)
    assert s.scroll(first + 1) is True
    assert (s.pos, s.line_offset) == ((0, 1), 1)
    assert s.window(1) == [stream[first + 1]]
    assert s.scroll(-2) is True
    assert (s.pos, s.line_offset) == ((0, 0), first - 1)
    assert s.scroll(-10_000) is True and (s.pos, s.line_offset) == ((0, 0), 0)
    assert s.scroll(-1) is False
    assert s.scroll(len(stream)) is False  # no such line: stay put
    assert (s.pos, s.line_offset) == ((0, 0), 0)
    assert s.scroll(len(stream) - 1) is True and s.window(10) == [stream[-1]]


def test_setting_pos_resets_line_offset():
    s = mk()
    s.scroll(2)
    assert s.line_offset == 2
    s.pos = (0, 1)
    assert s.line_offset == 0


def test_numbering_does_not_render_the_paragraphs_above(monkeypatch):
    # jumping deep into a long text must stay quick: lines above are counted, not built
    s = mk(make_book(200), noise=3)
    rendered = []
    original = s.mask.render
    monkeypatch.setattr(s.mask, "render", lambda *a, **k: rendered.append(a[1]) or original(*a, **k))
    lines = s.paragraph_lines((0, 199))
    assert rendered == [(0, 199)]
    fresh = mk(make_book(200), noise=3)
    total = sum(len(fresh.paragraph_lines((0, i))) for i in range(199))
    header = len(fresh.mask.header(None, 80))
    assert int(lines[0].plain[:4]) == header + total + 1
