import re
from random import Random

from fcc.book import Paragraph
from fcc.masks import get_mask
from fcc.masks.palette import GREEN

PARA = Paragraph("Oda dar ve basıktı, tavanı alçaktı. " * 6)


def test_header_is_a_diff_preamble():
    lines = [l.plain for l in get_mask("diff").header(Random(1), 80)]
    assert re.fullmatch(r"diff --git a/src/\S+ b/src/\S+", lines[0])
    assert any(l.startswith("--- a/src/") for l in lines)
    assert any(l.startswith("+++ b/src/") for l in lines)


def test_paragraph_starts_with_hunk_header_and_book_lines_are_additions():
    lines = get_mask("diff").render(PARA, (0, 0), 70, Random(1), 41)
    assert re.fullmatch(r"@@ -\d+,\d+ \+\d+,\d+ @@ def handler\(\):", lines[0].plain)
    assert not lines[0].is_book
    book = [l for l in lines if l.is_book]
    assert len(book) > 1
    assert all(l.plain.startswith("+ ") for l in book)
    assert all(l.segments[-1][1].split()[0] == "book" for l in book)
    assert GREEN in book[0].segments[-1][1]


def test_hunk_header_changes_with_line_no_start():
    m = get_mask("diff")
    a = m.render(PARA, (0, 0), 70, Random(1), 1)[0].plain
    b = m.render(PARA, (0, 0), 70, Random(1), 400)[0].plain
    assert a != b


def test_noise_is_context_or_removal_lines():
    m = get_mask("diff")
    rng = Random(4)
    kinds = {m.noise(rng, 70).plain[:2] for _ in range(100)}
    assert kinds == {"  ", "- "}
    assert all(not m.noise(rng, 70).is_book for _ in range(20))


def test_progress_label():
    assert re.fullmatch(r"Diffing \[=*>? *\] 142/380", get_mask("diff").progress_label(142, 380, 50))
