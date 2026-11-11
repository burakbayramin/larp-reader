from __future__ import annotations

import shutil
from pathlib import Path

import mobi

from fcc.book import Book, book_id
from fcc.errors import DrmProtected, UnsupportedFormat
from fcc.loaders.epub import EpubLoader
from fcc.loaders.html import HtmlLoader


def _encryption_type(data: bytes) -> int | None:
    """PalmDB record 0 -> MOBI header encryption field (0 none, 1/2 DRM); None if unparsable."""
    if len(data) < 82 or data[60:68] != b"BOOKMOBI":
        return None
    rec0 = int.from_bytes(data[78:82], "big")
    if rec0 + 14 > len(data):
        return None
    return int.from_bytes(data[rec0 + 12 : rec0 + 14], "big")


class Azw3Loader:
    name = "azw3"
    extensions = (".azw", ".azw3")

    def can_load(self, path: Path, head: bytes) -> bool:
        return path.suffix.lower() in self.extensions

    def load(self, path: Path) -> Book:
        data = path.read_bytes()
        if _encryption_type(data):
            raise DrmProtected("DRM korumalı, açılamaz")
        try:
            tempdir, extracted = mobi.extract(str(path))
        except Exception as e:
            raise UnsupportedFormat(f"dosya açılamadı: {e}") from e
        try:
            out = Path(extracted)
            suffix = out.suffix.lower()
            if suffix == ".epub":
                inner = EpubLoader().load(out)
            elif suffix in HtmlLoader.extensions:
                inner = HtmlLoader().load(out)
            else:
                raise UnsupportedFormat(f"desteklenmeyen çıktı: {suffix}")
        finally:
            shutil.rmtree(tempdir, ignore_errors=True)
        return Book(book_id(data), inner.chapters)
