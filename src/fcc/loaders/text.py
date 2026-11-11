from __future__ import annotations

import re
from pathlib import Path

from fcc.book import Book, Chapter, Paragraph, book_id
from fcc.text import normalize_paragraphs

_HEADING = re.compile(r"^#{1,2}\s+(.*\S)\s*$")


def _decode(data: bytes) -> str:
    for enc in ("utf-8-sig", "cp1254"):
        try:
            return data.decode(enc)
        except UnicodeDecodeError:
            continue
    return data.decode("latin-1")


def _chapter(title: str | None, lines: list[str]) -> Chapter:
    blocks = re.split(r"\n\s*\n", "\n".join(lines))
    paras = normalize_paragraphs(blocks)
    return Chapter(title, tuple(Paragraph(p) for p in paras))


class TextLoader:
    name = "text"
    extensions = (".txt", ".md", ".markdown")

    def can_load(self, path: Path, head: bytes) -> bool:
        return path.suffix.lower() in self.extensions

    def load(self, path: Path) -> Book:
        data = path.read_bytes()
        text = _decode(data).replace("\r\n", "\n").replace("\r", "\n")
        if path.suffix.lower() == ".txt":
            chapters = [_chapter(None, text.split("\n"))]
        else:
            chapters = []
            title: str | None = None
            buf: list[str] = []
            for line in text.split("\n"):
                m = _HEADING.match(line)
                if m:
                    chapters.append(_chapter(title, buf))
                    title, buf = m.group(1), []
                else:
                    buf.append(line)
            chapters.append(_chapter(title, buf))
            # drop an empty preamble (text before the first heading)
            chapters = [c for c in chapters if c.paragraphs or c.title is not None]
        return Book(book_id(data), tuple(chapters))
