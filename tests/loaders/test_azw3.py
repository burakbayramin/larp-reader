from __future__ import annotations

from pathlib import Path

import mobi
import pytest

from fcc.errors import DrmProtected
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


def test_azw3_mobi_error_is_drm(tmp_path, monkeypatch):
    f = tmp_path / "x.azw3"
    f.write_bytes(b"junk")

    def boom(path):
        raise ValueError("DRM")

    monkeypatch.setattr(mobi, "extract", boom)
    with pytest.raises(DrmProtected, match="DRM korumalı, açılamaz"):
        Azw3Loader().load(f)


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
