import pytest

from fcc.errors import EmptyBook, UnsupportedFormat
from fcc.loaders import load_book


def test_utf8_txt(tmp_path):
    p = tmp_path / "a.txt"
    p.write_text("Birinci paragraf\nikinci satır.\n\n\nİkinci paragraf.", encoding="utf-8")
    b = load_book(p)
    assert len(b.chapters) == 1 and b.chapters[0].title is None
    assert [x.text for x in b.chapters[0].paragraphs] == ["Birinci paragraf ikinci satır.", "İkinci paragraf."]
    assert len(b.id) == 16


def test_cp1254_txt_turkish(tmp_path):
    p = tmp_path / "a.txt"
    p.write_bytes("Şişli'de ğüıöç".encode("cp1254"))
    assert load_book(p).paragraph((0, 0)).text == "Şişli'de ğüıöç"


def test_md_headings_start_chapters(tmp_path):
    p = tmp_path / "a.md"
    p.write_text("# Bir\n\nmetin\n\n# İki\n\nmetin2", encoding="utf-8")
    b = load_book(p)
    assert [c.title for c in b.chapters] == ["Bir", "İki"]
    assert b.chapters[1].paragraphs[0].text == "metin2"


def test_empty_file_raises_empty_book(tmp_path):
    p = tmp_path / "a.txt"
    p.write_text("  \n\n", encoding="utf-8")
    with pytest.raises(EmptyBook):
        load_book(p)


def test_unknown_extension_raises_unsupported(tmp_path):
    p = tmp_path / "a.xyz"
    p.write_bytes(b"\x00\x01binary")
    with pytest.raises(UnsupportedFormat):
        load_book(p)
