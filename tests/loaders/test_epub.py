from __future__ import annotations

import zipfile
from pathlib import Path

import pytest
from ebooklib import epub

from fcc.errors import DrmProtected
from fcc.loaders import LOADERS, load_book
from fcc.loaders.epub import EpubLoader

PNG = bytes.fromhex(
    "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c489"
    "0000000d49444154789c6360f8cfc000000301010018dd8db00000000049454e44ae426082"
)


def _make_epub(path: Path) -> Path:
    book = epub.EpubBook()
    book.set_identifier("id-1")
    book.set_title("Deneme")
    book.set_language("tr")
    img = epub.EpubItem(uid="img", file_name="images/kapak.png", media_type="image/png", content=PNG)
    book.add_item(img)
    c1 = epub.EpubHtml(title="Birinci", file_name="c1.xhtml", lang="tr")
    c1.content = "<h1>Birinci Bölüm</h1><p>İlk bölümün metni: ığşçöü.</p><p><img src='images/kapak.png'/></p>"
    c2 = epub.EpubHtml(title="İkinci", file_name="c2.xhtml", lang="tr")
    c2.content = "<h2>İkinci Bölüm</h2><p>İkinci bölümün metni şurada.</p>"
    empty = epub.EpubHtml(title="Boş", file_name="empty.xhtml", lang="tr")
    empty.content = "<div>  </div>"
    chapters = [c1, empty, c2]
    for c in chapters:
        book.add_item(c)
    book.toc = (c1, c2)
    book.add_item(epub.EpubNcx())
    book.add_item(epub.EpubNav())
    book.spine = ["nav", *chapters]
    epub.write_epub(str(path), book)
    return path


def _with_encryption(src: Path, dst: Path, algorithm: str, uri: str) -> Path:
    xml = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        '<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" '
        'xmlns:enc="http://www.w3.org/2001/04/xmlenc#">'
        f'<enc:EncryptedData><enc:EncryptionMethod Algorithm="{algorithm}"/>'
        f'<enc:CipherData><enc:CipherReference URI="{uri}"/></enc:CipherData>'
        "</enc:EncryptedData></encryption>"
    )
    with zipfile.ZipFile(src) as zin, zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED) as zout:
        for item in zin.infolist():
            zout.writestr(item, zin.read(item.filename))
        zout.writestr("META-INF/encryption.xml", xml)
    return dst


@pytest.fixture
def epub_file(tmp_path) -> Path:
    return _make_epub(tmp_path / "book.epub")


def test_registry_order():
    assert [l.name for l in LOADERS] == ["text", "epub", "html", "azw3", "mupdf"]


def test_epub_spine_order_and_titles(epub_file):
    book = EpubLoader().load(epub_file)
    # nav page is skipped (nav content ignored); empty item is dropped
    assert [c.title for c in book.chapters] == ["Birinci Bölüm", "İkinci Bölüm"]
    assert book.chapters[0].paragraphs[0].text == "İlk bölümün metni: ığşçöü."
    assert book.chapters[1].paragraphs[0].text == "İkinci bölümün metni şurada."


def test_epub_image_becomes_asset_line(epub_file):
    book = EpubLoader().load(epub_file)
    texts = [p.text for p in book.chapters[0].paragraphs]
    assert "[asset: kapak.png]" in texts


def test_epub_goes_through_load_book(epub_file):
    assert len(load_book(epub_file).chapters) == 2


def test_epub_with_encryption_xml_raises_drm(epub_file, tmp_path):
    enc = _with_encryption(epub_file, tmp_path / "drm.epub", "http://www.w3.org/2001/04/xmlenc#aes128-cbc", "c1.xhtml")
    with pytest.raises(DrmProtected, match="DRM korumalı, açılamaz"):
        EpubLoader().load(enc)
    with pytest.raises(DrmProtected):
        load_book(enc)


@pytest.mark.parametrize(
    "algorithm",
    ["http://www.idpf.org/2008/embedding", "http://ns.adobe.com/pdf/enc#RC"],
)
def test_epub_font_obfuscation_is_not_drm(epub_file, tmp_path, algorithm):
    enc = _with_encryption(epub_file, tmp_path / "font.epub", algorithm, "fonts/a.otf")
    assert EpubLoader().load(enc).paragraph_count() >= 3
