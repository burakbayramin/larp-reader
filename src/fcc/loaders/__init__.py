from __future__ import annotations

from pathlib import Path

from fcc.book import Book
from fcc.errors import DrmProtected, EmptyBook, FccError, NoTextLayer, UnsupportedFormat
from fcc.loaders.base import Loader
from fcc.loaders.mupdf import MupdfLoader
from fcc.loaders.text import TextLoader

LOADERS: list[Loader] = [TextLoader(), MupdfLoader()]


def load_book(path: Path) -> Book:
    path = Path(path)
    with path.open("rb") as f:
        head = f.read(4096)
    for loader in LOADERS:
        if not loader.can_load(path, head):
            continue
        try:
            book = loader.load(path)
        except (DrmProtected, NoTextLayer):
            raise
        except Exception:
            continue
        if book.paragraph_count() == 0:
            raise EmptyBook("dosyada okunabilir metin bulunamadı")
        return book
    raise UnsupportedFormat(f"desteklenmeyen dosya biçimi: {path.suffix or path.name}")
