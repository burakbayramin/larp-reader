from random import Random

import pytest

from fcc.book import Paragraph
from fcc.errors import NoTextLayer
from fcc.masks import MASK_NAMES, get_mask, registered_mask_names

PARA = Paragraph("Raskolnikov o sabah erkenden uyandı. Oda dar ve basıktı, " * 3)
FORBIDDEN = ("book", "reader", "okuyucu")


def extract_words(lines):
    words = []
    for line in lines:
        if line.is_book:
            for text, style in line.segments:
                if style.split()[:1] == ["book"]:
                    words += text.split()
    return words


@pytest.mark.parametrize("name", registered_mask_names())
def test_render_keeps_every_word_in_order(name):
    lines = get_mask(name).render(PARA, (0, 0), 60, Random(1))
    assert extract_words(lines) == PARA.text.split()


@pytest.mark.parametrize("name", registered_mask_names())
def test_lines_fit_width(name):
    lines = get_mask(name).render(PARA, (0, 0), 60, Random(1))
    assert lines
    assert all(len(l.plain) <= 60 for l in lines)


@pytest.mark.parametrize("name", registered_mask_names())
def test_book_lines_carry_pos(name):
    lines = get_mask(name).render(PARA, (0, 0), 60, Random(1))
    assert any(l.is_book for l in lines)
    assert all((l.pos == (0, 0)) == l.is_book for l in lines)


@pytest.mark.parametrize("name", registered_mask_names())
def test_error_shows_real_reason(name):
    reason = "Metin katmanı yok, OCR desteklenmiyor"
    lines = get_mask(name).error(NoTextLayer(reason), 60)
    assert reason in " ".join(l.plain for l in lines)


@pytest.mark.parametrize("name", registered_mask_names())
def test_no_forbidden_words_in_chrome(name):
    mask = get_mask(name)
    rng = Random(3)
    chrome = [l.plain for l in mask.header(rng)]
    chrome += [mask.noise(rng, 50).plain for _ in range(50)]
    chrome.append(mask.progress_label(142, 380, 50))
    text = "\n".join(chrome).lower()
    assert not any(w in text for w in FORBIDDEN)


@pytest.mark.parametrize("name", registered_mask_names())
def test_render_tiny_width_does_not_hang(name):
    get_mask(name).render(PARA, (0, 0), 3, Random(1))


def test_unregistered_and_unknown_names_raise():
    assert "log" in MASK_NAMES
    for name in ("nope",):
        with pytest.raises(ValueError):
            get_mask(name)
