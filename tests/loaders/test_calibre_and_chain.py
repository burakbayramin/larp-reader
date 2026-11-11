from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest
from ebooklib import epub

from fcc.book import book_id
from fcc.errors import DrmProtected, UnsupportedFormat
from fcc.loaders import LOADERS, load_book
from fcc.loaders.calibre import CalibreLoader

EBOOK_CONVERT = "C:/calibre/ebook-convert.exe"


def _write_epub(path: Path) -> None:
    book = epub.EpubBook()
    book.set_identifier("id-cal")
    book.set_title("Calibre")
    book.set_language("tr")
    c1 = epub.EpubHtml(title="Bir", file_name="c1.xhtml", lang="tr")
    c1.content = "<h1>Bir</h1><p>Calibre ile dönüştürülen metin.</p>"
    book.add_item(c1)
    book.add_item(epub.EpubNcx())
    book.add_item(epub.EpubNav())
    book.spine = ["nav", c1]
    epub.write_epub(str(path), book)


@pytest.fixture
def calibre(monkeypatch):
    """Calibre 'installed'; subprocess.run mocked to write an EPUB to the output path."""
    calls: list[dict] = []

    def fake_run(cmd, **kwargs):
        calls.append({"cmd": cmd, **kwargs})
        _write_epub(Path(cmd[2]))
        return subprocess.CompletedProcess(cmd, 0, b"", b"")

    monkeypatch.setattr(shutil, "which", lambda name: EBOOK_CONVERT)
    monkeypatch.setattr(subprocess, "run", fake_run)
    return calls


@pytest.fixture
def no_calibre(monkeypatch):
    monkeypatch.setattr(shutil, "which", lambda name: None)

    def boom(*a, **k):
        raise AssertionError("subprocess.run must not be called")

    monkeypatch.setattr(subprocess, "run", boom)


def test_calibre_is_last_loader():
    assert isinstance(LOADERS[-1], CalibreLoader)


def test_calibre_used_when_others_fail(tmp_path, calibre):
    p = tmp_path / "kitap.djvu"
    p.write_bytes(b"AT&TFORM fake djvu")
    book = load_book(p)
    assert book.paragraph_count() > 0
    assert any("Calibre ile" in para.text for ch in book.chapters for para in ch.paragraphs)
    call = calibre[0]
    assert call["cmd"][0] == EBOOK_CONVERT
    assert call["cmd"][1] == str(p)
    assert call["cmd"][2].endswith(".epub")
    assert call["timeout"] == 120
    assert call["capture_output"] is True
    assert not call.get("shell")


def test_calibre_book_id_from_original_bytes(tmp_path, calibre):
    p = tmp_path / "kitap.djvu"
    p.write_bytes(b"AT&TFORM original bytes")
    assert load_book(p).id == book_id(p.read_bytes())


def test_calibre_temp_dir_cleaned_up(tmp_path, calibre):
    p = tmp_path / "kitap.djvu"
    p.write_bytes(b"x")
    load_book(p)
    assert not Path(calibre[0]["cmd"][2]).parent.exists()


def test_calibre_absent_gives_unsupported(tmp_path, no_calibre):
    p = tmp_path / "kitap.djvu"
    p.write_bytes(b"AT&TFORM fake djvu")
    with pytest.raises(UnsupportedFormat) as ei:
        load_book(p)
    msg = str(ei.value)
    assert ".djvu" in msg and "ebook-convert" in msg


def test_calibre_nonzero_exit_is_unsupported(tmp_path, monkeypatch):
    monkeypatch.setattr(shutil, "which", lambda n: EBOOK_CONVERT)
    monkeypatch.setattr(subprocess, "run", lambda cmd, **k: subprocess.CompletedProcess(cmd, 1, b"", b"boom"))
    p = tmp_path / "a.djvu"
    p.write_bytes(b"x")
    with pytest.raises(UnsupportedFormat) as ei:
        CalibreLoader().load(p)
    assert "1" in str(ei.value)


def test_calibre_timeout_is_unsupported(tmp_path, monkeypatch):
    def run(cmd, **k):
        raise subprocess.TimeoutExpired(cmd, 120)

    monkeypatch.setattr(shutil, "which", lambda n: EBOOK_CONVERT)
    monkeypatch.setattr(subprocess, "run", run)
    p = tmp_path / "a.djvu"
    p.write_bytes(b"x")
    with pytest.raises(UnsupportedFormat):
        CalibreLoader().load(p)


def test_calibre_missing_output_is_unsupported(tmp_path, monkeypatch):
    monkeypatch.setattr(shutil, "which", lambda n: EBOOK_CONVERT)
    monkeypatch.setattr(subprocess, "run", lambda cmd, **k: subprocess.CompletedProcess(cmd, 0, b"", b""))
    p = tmp_path / "a.djvu"
    p.write_bytes(b"x")
    with pytest.raises(UnsupportedFormat):
        CalibreLoader().load(p)


def test_calibre_can_load_follows_which(tmp_path, monkeypatch):
    monkeypatch.setattr(shutil, "which", lambda n: EBOOK_CONVERT)
    assert CalibreLoader().can_load(tmp_path / "a.xyz", b"")
    monkeypatch.setattr(shutil, "which", lambda n: None)
    assert not CalibreLoader().can_load(tmp_path / "a.xyz", b"")


def test_drm_not_retried_with_calibre(tmp_path, monkeypatch):
    class DrmLoader:
        name = "drm"
        extensions = (".djvu",)

        def can_load(self, path, head):
            return True

        def load(self, path):
            raise DrmProtected("DRM korumalı, açılamaz")

    def boom(*a, **k):
        raise AssertionError("calibre must not run after DRM")

    monkeypatch.setattr(shutil, "which", lambda n: EBOOK_CONVERT)
    monkeypatch.setattr(subprocess, "run", boom)
    monkeypatch.setattr("fcc.loaders.LOADERS", [DrmLoader(), CalibreLoader()])
    p = tmp_path / "a.djvu"
    p.write_bytes(b"x")
    with pytest.raises(DrmProtected):
        load_book(p)


def test_final_error_chains_last_loader_exception(tmp_path, no_calibre):
    p = tmp_path / "bad.docx"
    p.write_bytes(b"PK\x03\x04" + b"junk" * 50)
    with pytest.raises(UnsupportedFormat) as ei:
        load_book(p)
    assert ei.value.__cause__ is not None
    assert ".docx" in str(ei.value)
    assert "ebook-convert" in str(ei.value)


def test_final_error_without_matching_loader_has_no_cause(tmp_path, no_calibre):
    p = tmp_path / "a.djvu"
    p.write_bytes(b"x")
    with pytest.raises(UnsupportedFormat) as ei:
        load_book(p)
    assert ei.value.__cause__ is None


def test_no_install_hint_when_calibre_present_but_failed(tmp_path, monkeypatch):
    monkeypatch.setattr(shutil, "which", lambda n: EBOOK_CONVERT)
    monkeypatch.setattr(subprocess, "run", lambda cmd, **k: subprocess.CompletedProcess(cmd, 2, b"", b""))
    p = tmp_path / "a.djvu"
    p.write_bytes(b"x")
    with pytest.raises(UnsupportedFormat) as ei:
        load_book(p)
    assert "kurarak" not in str(ei.value)
    assert ".djvu" in str(ei.value)


@pytest.mark.parametrize(
    "name,content",
    [
        ("bad.docx", b"PK\x03\x04" + b"junk" * 100),
        ("bad.odt", b"PK\x03\x04" + b"junk" * 100),
        ("bad.epub", b"PK\x03\x04" + b"junk" * 100),
        ("bad.rtf", b"{\rtf1" + os.urandom(256)),
    ],
)
def test_corrupt_but_sniffable_is_unsupported(tmp_path, no_calibre, name, content):
    p = tmp_path / name
    p.write_bytes(content)
    with pytest.raises(UnsupportedFormat):
        load_book(p)
