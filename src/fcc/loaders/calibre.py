from __future__ import annotations

import shutil
import subprocess
import tempfile
from pathlib import Path

from fcc.book import Book, book_id
from fcc.errors import UnsupportedFormat
from fcc.loaders.epub import EpubLoader

_TIMEOUT_SECONDS = 120


class CalibreLoader:
    """Last resort: convert with Calibre's `ebook-convert` to EPUB, then read that."""

    name = "calibre"
    extensions: tuple[str, ...] = ()

    def can_load(self, path: Path, head: bytes) -> bool:
        return shutil.which("ebook-convert") is not None

    def load(self, path: Path) -> Book:
        exe = shutil.which("ebook-convert")
        if exe is None:
            raise UnsupportedFormat("ebook-convert bulunamadı")
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "converted.epub"
            try:
                proc = subprocess.run(
                    [exe, str(path), str(out)], timeout=_TIMEOUT_SECONDS, capture_output=True
                )
            except subprocess.TimeoutExpired as e:
                raise UnsupportedFormat(f"ebook-convert {_TIMEOUT_SECONDS} sn içinde bitmedi") from e
            except OSError as e:
                raise UnsupportedFormat(f"ebook-convert çalıştırılamadı: {e}") from e
            if proc.returncode != 0:
                raise UnsupportedFormat(f"ebook-convert başarısız (çıkış kodu {proc.returncode})")
            if not out.is_file():
                raise UnsupportedFormat("ebook-convert çıktı üretmedi")
            book = EpubLoader().load(out)
        return Book(book_id(path.read_bytes()), book.chapters)
