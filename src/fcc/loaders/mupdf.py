from __future__ import annotations

import re
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path

import pymupdf

from fcc.book import Book, Chapter, Paragraph, book_id
from fcc.errors import DrmProtected, NoTextLayer, UnsupportedFormat
from fcc.text import normalize_paragraphs

_ZIP_EXTS = (".cbz", ".epub")
_MIN_PAGES_FOR_HEADER_STRIP = 3


def _fb2_chapters(data: bytes) -> list[Chapter]:
    """FB2 has no TOC in MuPDF; read <section> structure directly."""

    def local(tag: str) -> str:
        return tag.rsplit("}", 1)[-1]

    def text_of(el: ET.Element) -> str:
        return " ".join("".join(el.itertext()).split())

    root = ET.fromstring(data)
    chapters: list[Chapter] = []
    for body in (e for e in root if local(e.tag) == "body" and e.get("name") != "notes"):
        for sec in (e for e in body.iter() if local(e.tag) == "section"):
            ps = [text_of(c) for c in sec if local(c.tag) == "p"]
            if not ps:
                continue
            title_el = next((c for c in sec if local(c.tag) == "title"), None)
            title = text_of(title_el) if title_el is not None else None
            paras = normalize_paragraphs(ps)
            chapters.append(Chapter(title or None, tuple(Paragraph(p) for p in paras)))
    return chapters


def _mask(text: str) -> str:
    return re.sub(r"\d", "#", " ".join(text.split()))


def _page_blocks(page: pymupdf.Page) -> list[tuple]:
    return sorted(page.get_text("blocks"), key=lambda b: (b[5],))


def _repeated_keys(pages: list[list[tuple]]) -> set[str]:
    """Masked texts that appear as first/last text block on >= 50% of pages."""
    counts: Counter[str] = Counter()
    for blocks in pages:
        texts = [b for b in blocks if b[6] == 0 and b[4].strip()]
        if not texts:
            continue
        keys = {_mask(texts[0][4]), _mask(texts[-1][4])}
        counts.update(keys)
    return {k for k, n in counts.items() if n * 2 >= len(pages)}


def _page_paragraphs(blocks: list[tuple], page_no: int, drop: set[str]) -> tuple[list[str], int]:
    """Return paragraphs for a page and the number of real text paragraphs among them."""
    texts = [b for b in blocks if b[6] == 0 and b[4].strip()]
    first, last = (texts[0], texts[-1]) if texts else (None, None)
    out: list[str] = []
    text_count = 0
    img_no = 0
    for b in blocks:
        if b[6] == 1:
            img_no += 1
            out.append(f"[asset: page{page_no}_img{img_no}]")
        elif b[6] == 0 and b[4].strip():
            if (b is first or b is last) and _mask(b[4]) in drop:
                continue
            norm = normalize_paragraphs([b[4].replace("\u00ad\n", "")])
            out.extend(norm)
            text_count += len(norm)
    return out, text_count


class MupdfLoader:
    name = "mupdf"
    extensions = (".pdf", ".xps", ".oxps", ".cbz", ".fb2", ".mobi", ".epub")

    def can_load(self, path: Path, head: bytes) -> bool:
        if path.suffix.lower() in self.extensions:
            return True
        return (
            head.startswith(b"%PDF")
            or head.startswith(b"PK\x03\x04")
            or b"<FictionBook" in head[:1024]
            or head[60:68] == b"BOOKMOBI"
        )

    def load(self, path: Path) -> Book:
        data = path.read_bytes()
        if path.suffix.lower() == ".fb2":
            try:
                chapters = _fb2_chapters(data)
            except ET.ParseError:
                chapters = []
            if chapters:
                return Book(book_id(data), tuple(chapters))
        try:
            doc = pymupdf.open(path)
        except Exception as e:
            raise UnsupportedFormat(f"dosya açılamadı: {e}") from e
        try:
            if doc.needs_pass or doc.is_encrypted:
                raise DrmProtected("DRM korumalı, açılamaz")
            try:
                pages = [_page_blocks(doc[i]) for i in range(doc.page_count)]
                toc = doc.get_toc()
            except Exception as e:
                raise UnsupportedFormat(f"dosya okunamadı: {e}") from e
            is_pdf = doc.is_pdf
        finally:
            doc.close()

        drop: set[str] = set()
        if is_pdf and len(pages) >= _MIN_PAGES_FOR_HEADER_STRIP:
            drop = _repeated_keys(pages)

        per_page: list[list[str]] = []
        total_text = 0
        for i, blocks in enumerate(pages):
            paras, n = _page_paragraphs(blocks, i + 1, drop)
            per_page.append(paras)
            total_text += n
        if total_text == 0:
            raise NoTextLayer("Metin katmanı yok, OCR desteklenmiyor")

        def chapter(title: str | None, start: int, end: int) -> Chapter:
            paras = [p for pg in per_page[start:end] for p in pg]
            return Chapter(title, tuple(Paragraph(p) for p in paras))

        n_pages = len(per_page)
        starts: dict[int, str | None] = {}
        for _, title, pg in toc:
            starts.setdefault(max(0, min(int(pg) - 1, n_pages - 1)), str(title).strip() or None)
        if not starts:
            chapters = [chapter(None, 0, n_pages)]
        else:
            bounds = sorted(starts)
            chapters = []
            if bounds[0] > 0:
                chapters.append(chapter(None, 0, bounds[0]))
            for k, s in enumerate(bounds):
                e = bounds[k + 1] if k + 1 < len(bounds) else n_pages
                chapters.append(chapter(starts[s], s, e))
        chapters = [c for c in chapters if c.paragraphs]
        return Book(book_id(data), tuple(chapters))
