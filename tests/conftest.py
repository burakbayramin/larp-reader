from __future__ import annotations

from pathlib import Path

import pymupdf
import pytest

_ARIAL = Path(r"C:\Windows\Fonts\arial.ttf")

BODY_1 = "Türkçe karakterler: ığşçöü İstanbul."
BODY_2 = "Ağır ağır çıkalım şu dağlara."
PAGE_WORDS = {2: "ikinci", 3: "üçüncü", 4: "dördüncü"}


def _font() -> pymupdf.Font:
    if _ARIAL.exists():
        return pymupdf.Font(fontfile=str(_ARIAL))
    return pymupdf.Font("notos")


def _write(page: pymupdf.Page, font: pymupdf.Font, point: tuple[float, float], text: str) -> None:
    page.insert_font(fontname="f0", fontbuffer=font.buffer)
    page.insert_text(point, text, fontname="f0", fontsize=11)


@pytest.fixture
def make_pdf(tmp_path):
    """Build a PDF with `pages` pages; each page has a body block, a hyphenated block and a footer."""

    def _make(pages: int = 3, footer: bool = True, name: str = "book.pdf") -> Path:
        font = _font()
        doc = pymupdf.open()
        for i in range(1, pages + 1):
            page = doc.new_page()
            _write(page, font, (72, 100), BODY_1 if i == 1 else f"{BODY_2} {PAGE_WORDS[i]}")
            _write(page, font, (72, 160), "bölüm-\nlendirme tamam.")
            if footer:
                _write(page, font, (72, 800), f"Sayfa {i}")
        path = tmp_path / name
        doc.save(path)
        doc.close()
        return path

    return _make


@pytest.fixture
def encrypted_pdf(tmp_path) -> Path:
    doc = pymupdf.open()
    doc.new_page().insert_text((72, 100), "gizli")
    path = tmp_path / "enc.pdf"
    doc.save(path, encryption=pymupdf.PDF_ENCRYPT_AES_256, user_pw="x", owner_pw="y")
    doc.close()
    return path


@pytest.fixture
def image_only_pdf(tmp_path) -> Path:
    pix = pymupdf.Pixmap(pymupdf.csRGB, pymupdf.IRect(0, 0, 40, 40), False)
    pix.set_rect(pix.irect, (200, 30, 30))
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_image(pymupdf.Rect(72, 72, 272, 272), pixmap=pix)
    path = tmp_path / "scan.pdf"
    doc.save(path)
    doc.close()
    return path


FB2 = """<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0">
<description><title-info><book-title>Deneme</book-title></title-info></description>
<body>
<section><title><p>Birinci Bölüm</p></title><p>İlk bölümün metni burada.</p></section>
<section><title><p>İkinci Bölüm</p></title><p>İkinci bölümün metni şurada.</p></section>
</body>
</FictionBook>
"""


@pytest.fixture
def fb2_file(tmp_path) -> Path:
    path = tmp_path / "book.fb2"
    path.write_text(FB2, encoding="utf-8")
    return path


@pytest.fixture
def text_image_pdf(tmp_path) -> Path:
    pix = pymupdf.Pixmap(pymupdf.csRGB, pymupdf.IRect(0, 0, 40, 40), False)
    pix.set_rect(pix.irect, (200, 30, 30))
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((72, 60), "Resimli sayfa metni.")
    page.insert_image(pymupdf.Rect(72, 100, 272, 300), pixmap=pix)
    path = tmp_path / "img.pdf"
    doc.save(path)
    doc.close()
    return path


@pytest.fixture
def toc_pdf(tmp_path) -> Path:
    doc = pymupdf.open()
    for i, word in enumerate(["alfa", "beta", "gama", "delta"], 1):
        doc.new_page().insert_text((72, 100), f"Sayfa metni {word}.")
    doc.set_toc([[1, "Bir", 1], [1, "Iki", 3]])
    path = tmp_path / "toc.pdf"
    doc.save(path)
    doc.close()
    return path


FB2_RICH = """<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">
<body>
<section><title><p>Ana</p></title>
<epigraph><p>Epigraf metni.</p></epigraph>
<p>Giriş.</p>
<subtitle>Alt başlık</subtitle>
<image l:href="#cover.jpg"/>
<poem><stanza><v>Dize bir</v><v>Dize iki</v></stanza></poem>
<cite><p>Alıntı metni.</p></cite>
<section><title><p>İç</p></title><p>İç metin.</p></section>
<p>Son paragraf.</p>
</section>
</body>
</FictionBook>
"""


@pytest.fixture
def fb2_rich_file(tmp_path) -> Path:
    path = tmp_path / "rich.fb2"
    path.write_text(FB2_RICH, encoding="utf-8")
    return path
