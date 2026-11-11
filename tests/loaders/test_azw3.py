from __future__ import annotations

import os
from pathlib import Path

import mobi
import pytest

from fcc.errors import DrmProtected, UnsupportedFormat
from fcc.loaders import load_book
from fcc.loaders.azw3 import Azw3Loader

FIXTURE = Path(__file__).parent.parent / "fixtures" / "sample.azw3"


def test_azw3_fixture(monkeypatch):
    if not FIXTURE.exists():
        pytest.skip("tests/fixtures/sample.azw3 missing")
    dirs: list[Path] = []
    real = mobi.extract

    def spy(path):
        d, f = real(path)
        dirs.append(Path(d))
        return d, f

    monkeypatch.setattr(mobi, "extract", spy)
    loader = Azw3Loader()
    assert loader.can_load(FIXTURE, b"")
    book = loader.load(FIXTURE)
    assert book.paragraph_count() > 20
    assert any("wallpaper" in p.text.lower() for c in book.chapters for p in c.paragraphs)
    assert len(dirs) == 1 and not dirs[0].exists()


def test_azw3_encrypted_header_is_drm(tmp_path):
    if not FIXTURE.exists():
        pytest.skip("tests/fixtures/sample.azw3 missing")
    data = bytearray(FIXTURE.read_bytes())
    rec0 = int.from_bytes(data[78:82], "big")
    assert data[rec0 + 12 : rec0 + 14] == b"\x00\x00"
    data[rec0 + 12 : rec0 + 14] = (2).to_bytes(2, "big")
    f = tmp_path / "drm.azw3"
    f.write_bytes(bytes(data))
    with pytest.raises(DrmProtected, match="DRM korumalı, açılamaz"):
        Azw3Loader().load(f)


def test_azw3_garbage_is_unsupported_not_drm(tmp_path):
    f = tmp_path / "junk.azw3"
    f.write_bytes(os.urandom(2048))
    with pytest.raises(UnsupportedFormat):
        load_book(f)


def test_azw3_html_output_uses_html_loader_and_cleans_up(tmp_path, monkeypatch):
    out = tmp_path / "out"
    out.mkdir()
    (out / "book.html").write_text("<h1>Ba</h1><p>Merhaba dünya.</p>", encoding="utf-8")
    f = tmp_path / "x.azw"
    f.write_bytes(b"data")
    monkeypatch.setattr(mobi, "extract", lambda p: (str(out), str(out / "book.html")))
    book = Azw3Loader().load(f)
    assert book.chapters[0].paragraphs[0].text == "Merhaba dünya."
    assert not out.exists()
