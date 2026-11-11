import re
from random import Random

from fcc.book import Paragraph
from fcc.masks import get_mask

PARA = Paragraph("Oda dar ve basıktı, tavanı alçaktı. " * 6)


def test_noise_is_compiling_line_with_semver():
    m = get_mask("build")
    rng = Random(6)
    for _ in range(50):
        n = m.noise(rng, 80)
        assert re.fullmatch(r"   Compiling [a-z_\-]+ v\d+\.\d+\.\d+", n.plain), n.plain
        assert not n.is_book


def test_book_lines_are_warning_note_lines():
    lines = get_mask("build").render(PARA, (0, 0), 70, Random(1), 41)
    plain = [l.plain for l in lines]
    assert any(p.startswith("warning:") for p in plain)
    assert any(re.fullmatch(r"\s+--> src/\S+\.rs:\d+:\d+", p) for p in plain)
    book = [l for l in lines if l.is_book]
    assert len(book) > 1
    assert all(l.plain.startswith("   |  ") for l in book)
    assert all(l.segments[-1][1].split()[0] == "book" for l in book)


def test_location_changes_with_line_no_start():
    m = get_mask("build")
    arrow = lambda s: [l.plain for l in m.render(PARA, (0, 0), 70, Random(1), s) if "-->" in l.plain]
    assert arrow(1) != arrow(400)


def test_progress_label():
    assert re.fullmatch(r"Building \[=*>? *\] 142/380", get_mask("build").progress_label(142, 380, 50))
