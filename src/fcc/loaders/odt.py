from __future__ import annotations

import zipfile
from pathlib import Path, PurePosixPath

from odf import teletype
from odf.draw import Image
from odf.opendocument import load as odf_load
from odf.namespaces import TEXTNS

from fcc.book import Book, Chapter, Paragraph, book_id
from fcc.errors import UnsupportedFormat

_MIMETYPE = b"application/vnd.oasis.opendocument.text"
_H = (TEXTNS, "h")
_P = (TEXTNS, "p")
_CONTAINERS = {(TEXTNS, "list"), (TEXTNS, "list-item"), (TEXTNS, "section")}


class OdtLoader:
    name = "odt"
    extensions = (".odt",)

    def can_load(self, path: Path, head: bytes) -> bool:
        if path.suffix.lower() not in self.extensions or head[:2] != b"PK":
            return False
        try:
            with zipfile.ZipFile(path) as z:
                return z.read("mimetype").strip() == _MIMETYPE
        except (OSError, KeyError, zipfile.BadZipFile):
            return False

    def load(self, path: Path) -> Book:
        data = path.read_bytes()
        try:
            doc = odf_load(str(path))
            root = doc.text
        except Exception as e:
            raise UnsupportedFormat(f"dosya açılamadı: {e}") from e
        chapters: list[Chapter] = []
        state: dict = {"title": None, "paras": []}

        def flush() -> None:
            if state["paras"]:
                chapters.append(Chapter(state["title"], tuple(state["paras"])))

        def walk(node) -> None:
            for child in node.childNodes:
                if child.nodeType != child.ELEMENT_NODE:
                    continue
                if child.qname == _H:
                    text = " ".join(teletype.extractText(child).split())
                    if text:
                        flush()
                        state["title"], state["paras"] = text, []
                elif child.qname == _P:
                    text = " ".join(teletype.extractText(child).split())
                    if text:
                        state["paras"].append(Paragraph(text))
                    for img in child.getElementsByType(Image):
                        href = img.getAttribute("href") or ""
                        name = PurePosixPath(href).name
                        if name:
                            state["paras"].append(Paragraph(f"[asset: {name}]"))
                elif child.qname in _CONTAINERS:
                    walk(child)

        walk(root)
        flush()
        return Book(book_id(data), tuple(chapters))
