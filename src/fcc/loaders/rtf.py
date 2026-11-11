from __future__ import annotations

import re
from pathlib import Path

from striprtf.striprtf import rtf_to_text

from fcc.book import Book, Chapter, Paragraph, book_id
from fcc.errors import UnsupportedFormat
from fcc.text import normalize_paragraphs

_ANSICPG = re.compile(rb"\\ansicpg(\d+)")


class RtfLoader:
    name = "rtf"
    extensions = (".rtf",)

    def can_load(self, path: Path, head: bytes) -> bool:
        return path.suffix.lower() in self.extensions and head.lstrip()[:5] == b"{\\rtf"

    def load(self, path: Path) -> Book:
        data = path.read_bytes()
        m = _ANSICPG.search(data[:4096])
        codepage = f"cp{m.group(1).decode()}" if m else "cp1252"
        try:
            raw = data.decode("latin-1")
            text = rtf_to_text(raw, encoding=codepage, errors="replace")
        except Exception as e:
            raise UnsupportedFormat(f"dosya açılamadı: {e}") from e
        paras = normalize_paragraphs(re.split(r"\n\s*\n|\r?\n", text))
        return Book(book_id(data), (Chapter(None, tuple(Paragraph(p) for p in paras)),))
