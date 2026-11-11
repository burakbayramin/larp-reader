from __future__ import annotations

import re
from pathlib import Path, PurePosixPath

from docx import Document
from docx.oxml.ns import qn
from docx.text.paragraph import Paragraph as DocxParagraph

from fcc.book import Book, Chapter, Paragraph, book_id
from fcc.errors import UnsupportedFormat

_HEADING = re.compile(r"^(heading|başlık|baslik)\s*[12]$", re.IGNORECASE)


def _is_heading(par: DocxParagraph) -> bool:
    style = par.style
    if style is None:
        return False
    return bool(_HEADING.match(style.name or "") or _HEADING.match(style.style_id or ""))


def _image_names(par: DocxParagraph) -> list[str]:
    names = []
    for blip in par._p.iter(qn("a:blip")):
        rid = blip.get(qn("r:embed"))
        if not rid or rid not in par.part.rels:
            continue
        target = par.part.rels[rid].target_ref
        names.append(PurePosixPath(target).name)
    return names


class DocxLoader:
    name = "docx"
    extensions = (".docx",)

    def can_load(self, path: Path, head: bytes) -> bool:
        return path.suffix.lower() in self.extensions and head[:2] == b"PK"

    def load(self, path: Path) -> Book:
        data = path.read_bytes()
        try:
            doc = Document(str(path))
            body = list(doc.element.body.iterchildren(qn("w:p")))
        except Exception as e:
            raise UnsupportedFormat(f"dosya açılamadı: {e}") from e
        chapters: list[Chapter] = []
        title: str | None = None
        paras: list[Paragraph] = []

        def flush() -> None:
            if paras:
                chapters.append(Chapter(title, tuple(paras)))

        for p in body:
            par = DocxParagraph(p, doc._body)
            text = " ".join(par.text.split())
            if text and _is_heading(par):
                flush()
                title, paras = text, []
                continue
            if text:
                paras.append(Paragraph(text))
            paras.extend(Paragraph(f"[asset: {n}]") for n in _image_names(par))
        flush()
        return Book(book_id(data), tuple(chapters))
