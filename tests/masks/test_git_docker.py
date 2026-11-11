"""The git log and docker compose views."""
from random import Random

from fcc.book import Paragraph
from fcc.masks import get_mask

PARA = Paragraph("Sabah erkenden uyandı ve pencereyi açtı. " * 4)


def test_git_paragraph_is_an_indented_commit_message():
    lines = get_mask("git").render(PARA, (0, 0), 80, Random(1), 10)
    assert lines[0].plain.startswith("commit ") and len(lines[0].plain) == 47
    assert lines[1].plain.startswith("Author: ")
    assert lines[2].plain.startswith("Date:   ")
    body = [l for l in lines if l.is_book]
    assert body and all(l.plain.startswith("    ") for l in body)


def test_git_history_goes_back_in_time():
    m = get_mask("git")
    first = m.render(PARA, (0, 0), 80, Random(1), 1)[2].plain
    later = m.render(PARA, (0, 1), 80, Random(1), 500)[2].plain
    assert first != later


def test_git_filler_is_a_stat_line():
    line = get_mask("git").noise(Random(2), 80)
    assert " | " in line.plain and line.plain.rstrip().endswith(("+", "-"))


def test_docker_lines_carry_a_container_prefix():
    m = get_mask("docker")
    lines = m.render(PARA, (0, 0), 80, Random(1))
    assert all(l.plain.startswith("worker-1 | ") for l in lines)
    noise = m.noise(Random(3), 80).plain
    assert " | " in noise
    assert m.header(Random(1), 80)[1].plain.startswith("Attaching to ")


def test_docker_narrow_pane_drops_the_service_name():
    lines = get_mask("docker").render(PARA, (0, 0), 25, Random(1))
    assert all(l.plain.startswith("| ") and len(l.plain) <= 25 for l in lines)
