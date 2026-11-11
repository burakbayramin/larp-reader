from __future__ import annotations

import shutil
from pathlib import Path

import mobi

from fcc.book import Book, book_id
from fcc.errors import DrmProtected, UnsupportedFormat
from fcc.loaders.epub import EpubLoader
from fcc.loaders.html import HtmlLoader


class Azw3Loader:
    name = "azw3"
    extensions = (".azw", ".azw3")

    def can_load(self, path: Path, head: bytes) -> bool:
        return path.suffix.lower() in self.extensions

    def load(self, path: Path) -> Book:
        try:
            tempdir, extracted = mobi.extract(str(path))
        except Exception as e:
            raise DrmProtected("DRM korumalı, açılamaz") from e
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
        return Book(book_id(path.read_bytes()), inner.chapters)
