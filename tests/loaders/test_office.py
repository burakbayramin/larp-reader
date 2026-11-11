from __future__ import annotations

import base64
import os

import pytest
from docx import Document
from odf.opendocument import OpenDocumentText
from odf.text import H, P
from odf.draw import Frame, Image

from fcc.errors import UnsupportedFormat
from fcc.loaders import load_book
from fcc.loaders.docx import DocxLoader
from fcc.loaders.odt import OdtLoader
from fcc.loaders.rtf import RtfLoader

PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
)


def test_docx_headings_make_chapters(tmp_path):
    d = Document()
    d.add_paragraph("preface text")
    d.add_heading("One", level=1)
    d.add_paragraph("alpha")
    d.add_paragraph("beta")
    d.add_heading("Two", level=2)
    d.add_paragraph("gamma")
    p = tmp_path / "b.docx"
    d.save(p)
    loader = DocxLoader()
    assert loader.can_load(p, p.read_bytes()[:4096])
    book = load_book(p)
    assert [c.title for c in book.chapters] == [None, "One", "Two"]
    assert [[x.text for x in c.paragraphs] for c in book.chapters] == [
        ["preface text"], ["alpha", "beta"], ["gamma"],
    ]


def test_docx_no_untitled_chapter_when_heading_first(tmp_path):
    d = Document()
    d.add_heading("One", level=1)
    d.add_paragraph("alpha")
    p = tmp_path / "b.docx"
    d.save(p)
    assert [c.title for c in load_book(p).chapters] == ["One"]


def test_docx_inline_image_becomes_asset(tmp_path):
    img = tmp_path / "pic.png"
    img.write_bytes(PNG)
    d = Document()
    d.add_paragraph("before")
    d.add_picture(str(img))
    d.add_paragraph("after")
    p = tmp_path / "b.docx"
    d.save(p)
    texts = [x.text for c in load_book(p).chapters for x in c.paragraphs]
    assert texts[0] == "before"
    assert texts[1].startswith("[asset: image") and texts[1].endswith(".png]")
    assert texts[2] == "after"


def _odt(tmp_path, build):
    doc = OpenDocumentText()
    build(doc)
    p = tmp_path / "b.odt"
    doc.save(str(p))
    return p


def test_odt_headings_make_chapters(tmp_path):
    def build(doc):
        doc.text.addElement(P(text="preface text"))
        doc.text.addElement(H(outlinelevel=1, text="One"))
        doc.text.addElement(P(text="alpha"))
        doc.text.addElement(H(outlinelevel=2, text="Two"))
        doc.text.addElement(P(text="beta"))

    p = _odt(tmp_path, build)
    assert OdtLoader().can_load(p, p.read_bytes()[:4096])
    book = load_book(p)
    assert [c.title for c in book.chapters] == [None, "One", "Two"]
    assert [[x.text for x in c.paragraphs] for c in book.chapters] == [
        ["preface text"], ["alpha"], ["beta"],
    ]


def test_odt_image_becomes_asset(tmp_path):
    def build(doc):
        f = tmp_path / "pic.png"
        f.write_bytes(PNG)
        href = doc.addPicture(str(f))
        fr = Frame(width="1cm", height="1cm")
        fr.addElement(Image(href=href))
        para = P()
        para.addElement(fr)
        doc.text.addElement(P(text="before"))
        doc.text.addElement(para)
        doc.text.addElement(P(text="after"))

    p = _odt(tmp_path, build)
    texts = [x.text for c in load_book(p).chapters for x in c.paragraphs]
    assert texts[0] == "before"
    assert texts[1].startswith("[asset: ") and texts[1].endswith(".png]")
    assert texts[2] == "after"


def test_odt_rejects_wrong_mimetype(tmp_path):
    import zipfile
    p = tmp_path / "x.odt"
    with zipfile.ZipFile(p, "w") as z:
        z.writestr("mimetype", "application/epub+zip")
    assert not OdtLoader().can_load(p, p.read_bytes()[:4096])


RTF = (
    r"{\rtf1\ansi\ansicpg1254\deff0{\fonttbl{\f0 Arial;}}\f0 "
    r"Merhaba d\'fcnya: \'fe\'f0\'fd \'dd\'de\'d0 \'e7\'f6\'fc\par"  # noqa
    "\r\n"
    r"Ikinci paragraf.\par}"
)


def test_rtf_turkish_chars(tmp_path):
    p = tmp_path / "b.rtf"
    p.write_bytes(RTF.encode("ascii"))
    assert RtfLoader().can_load(p, p.read_bytes()[:4096])
    book = load_book(p)
    texts = [x.text for c in book.chapters for x in c.paragraphs]
    assert texts == ["Merhaba dünya: şğı İŞĞ çöü", "Ikinci paragraf."]


@pytest.mark.parametrize("ext", [".docx", ".odt", ".rtf"])
def test_corrupt_office_file_is_unsupported(tmp_path, ext):
    p = tmp_path / f"bad{ext}"
    p.write_bytes(os.urandom(2048))
    with pytest.raises(UnsupportedFormat):
        load_book(p)
