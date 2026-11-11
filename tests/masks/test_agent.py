"""The agent view: a coding-agent session with edits, commands and narration."""
import zlib
from random import Random

import pytest

from fcc.book import Paragraph
from fcc.masks import get_mask
from fcc.masks.agent import ADD_BG, BULLET, DEL_BG, layout


def para_of_kind(kind: str, start: int = 0) -> Paragraph:
    """A paragraph whose layout is `kind` (the layout is picked from the text)."""
    for i in range(start, start + 500):
        text = f"Sabah {i} erkenden uyandı, pencereyi açtı ve dışarı baktı. " * 3
        if layout(text).kind == kind:
            return Paragraph(text)
    raise AssertionError(kind)


def book_lines(lines):
    return [l for l in lines if l.is_book]


def test_layout_is_picked_from_the_text_and_mixes_kinds():
    kinds = [layout(f"paragraf {i}").kind for i in range(200)]
    assert 0.55 < kinds.count("edit") / 200 < 0.85
    assert layout("aynı metin") == layout("aynı metin")


def test_edit_paragraph_is_a_diff_with_the_text_as_added_lines():
    m = get_mask("agent")
    lines = m.render(para_of_kind("edit"), (0, 0), 100, Random(1), 40)
    assert lines[0].plain.startswith(f"{BULLET} Update(")
    assert "⎿  Updated " in lines[1].plain and " with " in lines[1].plain
    added = book_lines(lines)
    assert added and all(l.plain[8:10] == "+ " for l in added)
    assert all(any(ADD_BG in style for _, style in l.segments) for l in added)
    removed = [l for l in lines if any(DEL_BG in style for _, style in l.segments)]
    assert len(removed) == layout(para_of_kind("edit").text).removed
    assert lines[-1].plain == ""


def test_edit_summary_counts_match_the_diff():
    para = para_of_kind("edit")
    lines = get_mask("agent").render(para, (0, 0), 100, Random(1), 40)
    adds, rems = len(book_lines(lines)), layout(para.text).removed
    summary = lines[1].plain
    assert f"{adds} addition" in summary and f"{rems} removal" in summary


def test_say_paragraph_is_narration():
    lines = get_mask("agent").render(para_of_kind("say"), (0, 0), 100, Random(1), 40)
    text = book_lines(lines)
    assert text[0].plain.startswith(f"{BULLET} ")
    assert all(l.plain.startswith("  ") for l in text[1:])
    assert lines[-1].plain == ""


def test_same_paragraph_draws_the_same_every_time():
    para = para_of_kind("edit")
    a = get_mask("agent").render(para, (0, 3), 90, Random(5), 120)
    b = get_mask("agent").render(para, (0, 3), 90, Random(5), 120)
    assert a == b


def test_diff_code_does_not_repeat_across_fifty_paragraphs():
    m = get_mask("agent")
    seen: list[str] = []
    start = 1
    for i in range(50):
        para = para_of_kind("edit", start=i * 7)
        lines = m.render(para, (0, i), 100, Random(i), start)
        start += len(lines)
        seen += [l.plain[10:].strip() for l in lines if not l.is_book and l.plain[:8].strip().isdigit()]
    assert len(seen) > 100
    assert len(seen) == len(set(seen)), [s for s in seen if seen.count(s) > 1][:5]


def _meaningful(line: str) -> bool:
    """Lines that carry content: a tool call's argument, a line of code, a to-do item."""
    s = line.strip()
    return (s.startswith(BULLET) and "(" in s) or line[:8].strip().isdigit() or "☐" in s or "☒" in s


def test_hidden_stream_does_not_repeat_within_300_lines():
    m = get_mask("agent")
    rng = Random(9)
    stream = [m.noise(rng, 100).plain for _ in range(3000)]
    meaningful = [(i, s) for i, s in enumerate(stream) if _meaningful(s)]
    last: dict[str, int] = {}
    for i, s in meaningful:
        if s in last:
            assert i - last[s] > 300, (s, i - last[s])
        last[s] = i


@pytest.mark.parametrize("count", range(11))
def test_noise_block_has_exact_size_and_whole_units(count):
    m = get_mask("agent")
    m.render(para_of_kind("edit"), (0, 0), 100, Random(1), 40)
    block = m.noise_block(count, Random(count), 100)
    assert len(block) == count
    assert not any(l.is_book for l in block)
    # a tool result never starts a block: every result follows its call
    if block:
        assert not block[0].plain.lstrip().startswith("⎿")


def test_lines_fit_and_diff_background_spans_the_width():
    lines = get_mask("agent").render(para_of_kind("edit"), (0, 0), 80, Random(1), 40)
    for l in lines:
        assert len(l.plain) <= 80
    assert all(len(l.plain) == 80 for l in book_lines(lines))


def test_header_and_status():
    m = get_mask("agent")
    assert m.header(Random(1), 80)[0].plain.startswith(">")
    assert "142/380" in m.progress_label(142, 380, 80)
