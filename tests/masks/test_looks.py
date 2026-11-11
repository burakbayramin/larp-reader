"""The masks have to look like the real thing: coloured, coherent, in order."""

import re
from datetime import datetime
from random import Random

import pytest

from fcc.book import Book, Chapter, Paragraph
from fcc.masks import get_mask
from fcc.masks.code import LANGS
from fcc.masks.palette import COMMENT, FUNC, KEYWORD, NUMBER, STRING, highlight
from fcc.session import Session

PARAS = tuple(Paragraph(f"Paragraf {i} " + "kelime " * 30) for i in range(8))
BOOK = Book("bk", (Chapter(None, PARAS),))


def styles(lines):
    return {style for ln in lines for _text, style in ln.segments}


def test_highlight_colours_tokens_differently():
    (line,) = highlight("python", 'def retry(limit: int = 5): return "x"')
    by_text = {text: style for text, style in line}
    assert by_text["def"] == KEYWORD
    assert by_text["retry"] == FUNC
    assert by_text["5"] == NUMBER
    assert by_text['"x"'] == STRING


@pytest.mark.parametrize("lang", LANGS)
def test_code_filler_is_syntax_coloured_not_one_colour(lang):
    s = Session(BOOK, get_mask("code", lang), noise=5, width=100)
    filler = [ln for i in range(8) for ln in s.paragraph_lines((0, i)) if not ln.is_book]
    code = [ln for ln in filler if len(ln.segments) > 1]
    assert code
    assert len(styles(code)) >= 5
    # a line of code mixes colours; it is not painted in one flat style
    assert any(len({st for _t, st in ln.segments[1:]}) >= 3 for ln in code)


@pytest.mark.parametrize("lang", LANGS)
def test_code_text_is_a_comment_in_the_editor_comment_colour(lang):
    lines = get_mask("code", lang).render(PARAS[0], (0, 0), 80, Random(1))
    text = [ln for ln in lines if ln.is_book]
    assert text
    assert all(ln.segments[-1][1] == f"book {COMMENT}" for ln in text)
    assert all(ln.segments[-2][1] == COMMENT for ln in text)  # the comment marker


@pytest.mark.parametrize("lang", ("rust", "typescript", "go", "java", "csharp"))
@pytest.mark.parametrize("count", range(11))
def test_code_filler_block_has_exact_size_and_no_cut_off_definition(lang, count):
    mask = get_mask("code", lang)
    for seed in range(20):
        block = mask.noise_block(count, Random(seed), 200)
        assert len(block) == count
        body = "\n".join(ln.plain[5:] for ln in block)
        assert body.count("{") == body.count("}"), body
        assert body.count("(") == body.count(")"), body


def test_code_filler_block_uses_whole_definitions_when_there_is_room():
    mask = get_mask("code", "rust")
    blocks = [mask.noise_block(10, Random(seed), 200) for seed in range(20)]
    assert all(any("{" in ln.plain for ln in b) for b in blocks)


TIME = re.compile(r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3})Z ")


def test_log_time_never_runs_backwards_across_text_and_filler():
    s = Session(BOOK, get_mask("log"), noise=5, width=120)
    times = []
    for i in range(8):
        for ln in s.paragraph_lines((0, i)):
            m = TIME.match(ln.plain)
            assert m, ln.plain
            times.append(datetime.strptime(m.group(1), "%Y-%m-%dT%H:%M:%S.%f"))
    assert len(times) > 40
    assert all(a < b for a, b in zip(times, times[1:]))


def test_log_levels_have_their_own_colours():
    mask = get_mask("log")
    rng = Random(3)
    level_style = {}
    for _ in range(200):
        ln = mask.noise(rng, 120)
        level_style.setdefault(ln.segments[1][0].strip(), set()).add(ln.segments[1][1])
    assert set(level_style) == {"INFO", "DEBUG", "WARN"}
    assert all(len(v) == 1 for v in level_style.values())
    assert len({next(iter(v)) for v in level_style.values()}) == 3


@pytest.mark.parametrize("name", ("log", "build", "test"))
def test_long_text_is_in_the_plain_foreground_colour(name):
    # hours of reading in warning-yellow or failure-red is not on
    from fcc.masks.palette import FG

    lines = get_mask(name).render(PARAS[0], (0, 0), 80, Random(1))
    text = [ln for ln in lines if ln.is_book]
    assert text and all(ln.segments[-1][1] == f"book {FG}" for ln in text)


@pytest.mark.parametrize("lang", LANGS)
def test_code_does_not_repeat_a_definition_on_one_screen(lang):
    # a real file does not define the same thing twice a few lines apart
    s = Session(BOOK, get_mask("code", lang), noise=3, width=100)
    for first in range(6):
        screen = [ln for i in (first, first + 1) for ln in s.paragraph_lines((0, i))]
        code = [ln.plain[5:] for ln in screen if not ln.is_book and len(ln.plain) > 12]
        signatures = [c for c in code if not c.startswith(("    ", "  ", "}", ")"))]
        assert len(signatures) == len(set(signatures)), signatures
