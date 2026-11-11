import re
from random import Random

import pytest

from fcc.book import Paragraph
from fcc.masks import get_mask
from fcc.masks.code import LANGS, CodeMask

PARA = Paragraph("Oda dar ve basıktı, tavanı alçaktı. " * 4)


def plain(lang, start=1):
    return [l.plain for l in get_mask("code", lang).render(PARA, (0, 0), 60, Random(1), start)]


def test_python_uses_hash_or_docstring():
    text = "\n".join(plain("python"))
    assert '"""' in text or "#" in text


def test_rust_uses_slash_comments():
    book = [l.plain for l in get_mask("code", "rust").render(PARA, (0, 0), 60, Random(1)) if l.is_book]
    assert book
    assert all("/// " in t for t in book)


def test_unknown_lang_raises_value_error():
    with pytest.raises(ValueError):
        CodeMask("cobol")
    with pytest.raises(ValueError):
        get_mask("code", "cobol")


def test_line_numbers_start_at_given_value_and_increase():
    lines = plain("python", 40)
    assert lines
    nums = [int(t[:4]) for t in lines]
    assert nums == list(range(40, 40 + len(lines)))
    assert plain("python", 40) == lines


@pytest.mark.parametrize("lang", LANGS)
def test_every_lang_renders_and_fits(lang):
    mask = get_mask("code", lang)
    assert mask.header(Random(1), 60)
    out = mask.render(PARA, (0, 0), 60, Random(1))
    assert out
    assert all(len(l.plain) <= 60 for l in out)


def test_progress_label_format():
    mask = get_mask("code")
    assert mask.progress_label(142, 380, 50) == "Indexing [===>      ] 142/380"
    assert mask.progress_label(0, 0, 50).startswith("Indexing [")


GUTTER = re.compile(r"^ *(\d+) ")


@pytest.mark.parametrize("lang", LANGS)
def test_every_line_has_gutter_including_noise(lang):
    from fcc.book import Book, Chapter
    from fcc.session import Session

    paras = tuple(Paragraph(f"Paragraf {i} " + "kelime " * 40) for i in range(6))
    s = Session(Book("bk", (Chapter(None, paras),)), get_mask("code", lang), noise=3, width=60)
    noisy = 0
    for i in range(6):
        block = s.paragraph_lines((0, i))
        noisy += len(block) - len(s.mask.render(paras[i], (0, i), 60, Random(0), 1))
        numbers = []
        for ln in block:
            m = GUTTER.match(ln.plain)
            assert m, repr(ln.plain)
            assert len(ln.plain) <= 60
            numbers.append(int(m.group(1)))
        assert numbers == sorted(numbers) and len(set(numbers)) == len(numbers)
    assert noisy > 0  # noise lines were actually produced and checked


def test_standalone_noise_lines_have_gutter():
    m = get_mask("code", "python")
    rng = Random(4)
    lines = [m.noise(rng, 30) for _ in range(20)]
    assert all(GUTTER.match(ln.plain) and len(ln.plain) <= 30 for ln in lines)
