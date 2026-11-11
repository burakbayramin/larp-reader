import re
from random import Random

from fcc.book import Paragraph
from fcc.masks import get_mask

PARA = Paragraph("Oda dar ve basıktı, tavanı alçaktı. " * 6)


def test_header_is_pytest_session_banner():
    text = "\n".join(l.plain for l in get_mask("test").header(Random(1), 80))
    assert "test session starts" in text
    assert re.search(r"collected \d+ items", text)


def test_book_lines_are_assertion_error_message_lines():
    lines = get_mask("test").render(PARA, (0, 0), 70, Random(1))
    book = [l for l in lines if l.is_book]
    assert len(book) > 1
    assert all(l.plain.startswith("E   ") for l in book)
    assert all(l.segments[-1][1].split()[0] == "book" for l in book)
    plain = [l.plain for l in lines]
    first = next(i for i, l in enumerate(lines) if l.is_book)
    assert any(p.startswith("E   AssertionError") for p in plain[:first])


def test_noise_is_passed_test_line_with_percentage():
    m = get_mask("test")
    rng = Random(5)
    for _ in range(50):
        n = m.noise(rng, 80)
        assert re.fullmatch(r"tests/test_\w+\.py::test_\w+ PASSED +\[ *\d+%\]", n.plain), n.plain
        assert not n.is_book


def test_progress_label():
    assert re.fullmatch(r"\[ *37%\] collected 380 items", get_mask("test").progress_label(142, 380, 50))
