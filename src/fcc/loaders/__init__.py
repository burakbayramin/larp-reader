from __future__ import annotations

import shutil
from pathlib import Path

from fcc.book import Book
from fcc.errors import DrmProtected, EmptyBook, FccError, NoTextLayer, UnsupportedFormat
from fcc.loaders.base import Loader
from fcc.loaders.azw3 import Azw3Loader
from fcc.loaders.calibre import CalibreLoader
from fcc.loaders.docx import DocxLoader
from fcc.loaders.epub import EpubLoader
from fcc.loaders.html import HtmlLoader
from fcc.loaders.mupdf import MupdfLoader
from fcc.loaders.odt import OdtLoader
from fcc.loaders.rtf import RtfLoader
from fcc.loaders.text import TextLoader

LOADERS: list[Loader] = [TextLoader(), EpubLoader(), HtmlLoader(), Azw3Loader(), DocxLoader(), OdtLoader(), RtfLoader(), MupdfLoader(), CalibreLoader()]


def load_book(path: Path) -> Book:
    path = Path(path)
    with path.open("rb") as f:
        head = f.read(4096)
    last_error: Exception | None = None
    for loader in LOADERS:
        if not loader.can_load(path, head):
            continue
        try:
            book = loader.load(path)
        except (DrmProtected, NoTextLayer):
            raise
        except Exception as e:
            last_error = e
            continue
        if book.paragraph_count() == 0:
            raise EmptyBook("dosyada okunabilir metin bulunamadı")
        return book
    ext = path.suffix or "(uzantısız)"  # never the file name: it may be the title
    msg = f'"{ext}" biçimi açılamadı'
    if shutil.which("ebook-convert") is None:
        msg += "; Calibre (ebook-convert) kurarak tekrar deneyin"
    raise UnsupportedFormat(msg) from last_error
