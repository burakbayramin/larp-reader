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
    assert mask.header(Random(1))
    out = mask.render(PARA, (0, 0), 60, Random(1))
    assert out
    assert all(len(l.plain) <= 60 for l in out)


def test_progress_label_format():
    mask = get_mask("code")
    assert mask.progress_label(142, 380, 50) == "Indexing [===>      ] 142/380"
    assert mask.progress_label(0, 0, 50).startswith("Indexing [")
