from __future__ import annotations

import pytest

from fcc.book import book_id
from fcc.errors import DrmProtected, NoTextLayer, UnsupportedFormat
from fcc.loaders import LOADERS, load_book
from fcc.loaders.mupdf import MupdfLoader


def _texts(book):
    return [p.text for c in book.chapters for p in c.paragraphs]


def test_pdf_paragraphs_turkish_text(make_pdf):
    path = make_pdf()
    book = MupdfLoader().load(path)
    texts = _texts(book)
    assert "Türkçe karakterler: ığşçöü İstanbul." in texts
    assert book.id == book_id(path.read_bytes())


def test_pdf_footer_removed(make_pdf):
    texts = _texts(MupdfLoader().load(make_pdf()))
    assert not any(t.startswith("Sayfa") for t in texts)
    assert any("Ağır ağır" in t for t in texts)


def test_pdf_footer_kept_when_not_repeated(make_pdf):
    # with only 2 pages header/footer stripping must not run
    texts = _texts(MupdfLoader().load(make_pdf(pages=2)))
    assert "Sayfa 1" in texts and "Sayfa 2" in texts


def test_one_page_pdf_single_block_survives(tmp_path):
    import pymupdf

    doc = pymupdf.open()
    doc.new_page().insert_text((72, 100), "Tek blok metin.")
    path = tmp_path / "one.pdf"
    doc.save(path)
    assert _texts(MupdfLoader().load(path)) == ["Tek blok metin."]


def test_pdf_hyphen_joined(make_pdf):
    texts = _texts(MupdfLoader().load(make_pdf()))
    assert "bölümlendirme tamam." in texts


def test_encrypted_pdf_raises_drm(encrypted_pdf):
    with pytest.raises(DrmProtected):
        MupdfLoader().load(encrypted_pdf)


def test_image_only_pdf_raises_no_text_layer(image_only_pdf):
    with pytest.raises(NoTextLayer):
        MupdfLoader().load(image_only_pdf)


def test_fb2_two_chapters(fb2_file):
    book = MupdfLoader().load(fb2_file)
    assert len(book.chapters) == 2
    assert book.chapters[0].title == "Birinci Bölüm"
    assert any("İlk bölümün metni" in p.text for p in book.chapters[0].paragraphs)
    assert any("İkinci bölümün metni" in p.text for p in book.chapters[1].paragraphs)


def test_load_book_routes_pdf_to_mupdf(make_pdf):
    assert any(isinstance(l, MupdfLoader) for l in LOADERS)
    book = load_book(make_pdf())
    assert book.paragraph_count() > 0


def test_corrupt_pdf_is_unsupported_format(tmp_path):
    path = tmp_path / "bad.pdf"
    path.write_bytes(bytes(range(256)) * 8)
    with pytest.raises(UnsupportedFormat):
        load_book(path)


def test_can_load_magic_and_extension(tmp_path):
    loader = MupdfLoader()
    assert loader.can_load(tmp_path / "a.pdf", b"")
    assert loader.can_load(tmp_path / "noext", b"%PDF-1.7")
    assert loader.can_load(tmp_path / "noext", b"<?xml version='1.0'?><FictionBook")
    assert loader.can_load(tmp_path / "noext", b"\0" * 60 + b"BOOKMOBI")
    assert not loader.can_load(tmp_path / "a.txt", b"hello")
