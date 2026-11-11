import re
from datetime import datetime
from random import Random

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
    lines = get_mask("log").render(PARA, (0, 0), 90, Random(1), 41)
    assert len(lines) > 3
    times = [ts(l) for l in lines]
    assert all(a < b for a, b in zip(times, times[1:]))


def test_book_lines_are_info_and_styled_as_book():
    lines = get_mask("log").render(PARA, (0, 0), 90, Random(1))
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
    noise = [m.noise(rng, 90) for _ in range(200)]
    levels = {LINE.match(n.plain).group(2).strip() for n in noise}
    assert levels <= {"INFO", "DEBUG", "WARN"} and len(levels) >= 2
    assert not any(n.is_book for n in noise)


def test_progress_label():
    assert re.fullmatch(r"Tailing \[=*>? *\] 142/380", get_mask("log").progress_label(142, 380, 50))
