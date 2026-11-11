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
    chrome = [l.plain for l in mask.header(rng, 80)]
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


@pytest.mark.parametrize("width", [40, 60, 120])
@pytest.mark.parametrize("name", registered_mask_names())
def test_header_fits_width(name, width):
    lines = get_mask(name).header(Random(1), width)
    assert lines
    assert all(len(l.plain) <= width for l in lines), [l.plain for l in lines]
    assert not any(l.is_book for l in lines)


@pytest.mark.parametrize("width", [40, 60, 80])
def test_test_banner_centered_to_width_like_pytest(width):
    banner = get_mask("test").header(Random(1), width)[0].plain
    assert len(banner) == width
    assert banner.strip("=").strip() == "test session starts"
    left, right = banner.split(" test session starts ")
    assert set(left) == set(right) == {"="} and abs(len(left) - len(right)) <= 1


@pytest.mark.parametrize("name", registered_mask_names())
@pytest.mark.parametrize("width", (40, 60, 120))
def test_line_count_matches_render(name, width):
    from fcc.masks.code import LANGS

    paras = [Paragraph("kelime " * n) for n in (1, 7, 40, 200)] + [Paragraph("x" * 300)]
    for lang in LANGS if name == "code" else ("python",):
        mask = get_mask(name, lang)
        for seed, para in enumerate(paras):
            assert mask.line_count(para, width) == len(mask.render(para, (0, 0), width, Random(seed)))
