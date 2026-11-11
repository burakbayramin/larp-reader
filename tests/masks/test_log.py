import re
from datetime import datetime
from random import Random

import pytest

from fcc.book import Paragraph
from fcc.masks import get_mask

PARA = Paragraph("Oda dar ve basıktı, tavanı alçaktı. " * 6)
LINE = re.compile(
    r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3})Z (INFO |DEBUG|WARN ) svc\.\w+ +\S"
)


def ts(line):
    m = LINE.match(line.plain)
    assert m, line.plain
    return datetime.strptime(m.group(1), "%Y-%m-%dT%H:%M:%S.%f")


def test_every_line_matches_format_and_timestamps_increase():
    lines = get_mask("log").render(PARA, (0, 0), 120, Random(1), 41)
    assert len(lines) > 3
    times = [ts(l) for l in lines]
    assert all(a < b for a, b in zip(times, times[1:]))


def test_book_lines_are_info_and_styled_as_book():
    lines = get_mask("log").render(PARA, (0, 0), 120, Random(1))
    book = [l for l in lines if l.is_book]
    assert book
    assert all(" INFO  svc.worker  " in l.plain for l in book)
    assert all(l.segments[-1][1].split()[0] == "book" for l in book)


def test_render_is_deterministic_and_depends_on_line_no_start():
    m = get_mask("log")
    a = [l.plain for l in m.render(PARA, (0, 0), 90, Random(1), 5)]
    assert a == [l.plain for l in m.render(PARA, (0, 0), 90, Random(1), 5)]
    assert a != [l.plain for l in m.render(PARA, (0, 0), 90, Random(1), 400)]


def test_noise_has_log_format_and_levels_vary():
    m = get_mask("log")
    rng = Random(2)
    noise = [m.noise(rng, 120) for _ in range(200)]
    levels = {LINE.match(n.plain).group(2).strip() for n in noise}
    assert levels <= {"INFO", "DEBUG", "WARN"} and len(levels) >= 2
    assert not any(n.is_book for n in noise)


def test_progress_label():
    assert re.fullmatch(r"Tailing \[=*>? *\] 142/380", get_mask("log").progress_label(142, 380, 50))


LONG = Paragraph("Oda dar ve basıktı, tavanı alçaktı; pencereden giren ışık azdı. " * 12)
FORMATS = {
    120: re.compile(r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3})Z (INFO |DEBUG|WARN ) svc\.\w+ +"),
    60: re.compile(r"^(\d{2}:\d{2}:\d{2}\.\d{3}) (INFO |DEBUG|WARN ) \w+ +"),
    40: re.compile(r"^(\d{2}:\d{2}:\d{2}) (INF|DBG|WRN) "),
}


@pytest.mark.parametrize("width", [40, 60, 120])
def test_prefix_adapts_to_width_and_every_line_fits(width):
    m = get_mask("log")
    rng = Random(5)
    lines = m.render(LONG, (0, 0), width, rng, 7) + [m.noise(rng, width) for _ in range(100)]
    for ln in lines:
        assert len(ln.plain) <= width, ln.plain
        assert FORMATS[width].match(ln.plain), ln.plain
    book = [ln for ln in lines if ln.is_book]
    body = [ln.segments[-1][0] for ln in book]
    prefix = len(book[0].plain) - len(body[0])
    assert width - prefix >= 20
    assert " ".join(body).split() == LONG.text.split()


@pytest.mark.parametrize("width", [40, 60])
def test_compact_timestamps_increase(width):
    lines = get_mask("log").render(LONG, (0, 0), width, Random(1), 41)
    stamps = [FORMATS[width].match(ln.plain).group(1) for ln in lines]
    assert stamps == sorted(stamps) and len(set(stamps)) > 1


def test_wide_terminal_keeps_full_iso_prefix():
    book = [ln for ln in get_mask("log").render(LONG, (0, 0), 100, Random(1)) if ln.is_book]
    assert all(" INFO  svc.worker  " in ln.plain for ln in book)
