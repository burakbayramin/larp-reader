from __future__ import annotations

from pathlib import Path
from urllib.parse import unquote

from bs4 import BeautifulSoup, NavigableString, Tag

from fcc.book import Book, Chapter, Paragraph, book_id
from fcc.loaders.text import _decode
from fcc.text import normalize_paragraphs

_SKIP = {"script", "style", "nav", "head"}
_BLOCKS = {"p", "div", "li", "blockquote", "ul", "ol", "section", "article", "body", "html",
           "h1", "h2", "h3", "h4", "h5", "h6", "br", "tr", "pre", "table"}


def _asset_name(src: str) -> str:
    return unquote(src.split("#", 1)[0].split("?", 1)[0]).rstrip("/").rsplit("/", 1)[-1]


def html_to_paragraphs(html: str) -> tuple[str | None, list[str]]:
    """Return (first h1/h2 text or None, paragraphs). Images become `[asset: <basename>]`."""
    soup = BeautifulSoup(html, "html.parser")
    title: str | None = None
    out: list[str] = []
    buf: list[str] = []

    def flush() -> None:
        if buf:
            out.extend(normalize_paragraphs(["".join(buf)]))
            buf.clear()

    def walk(node: Tag) -> None:
        nonlocal title
        for child in node.children:
            if isinstance(child, NavigableString):
                if type(child).__name__ in ("Comment", "Doctype", "Declaration", "ProcessingInstruction", "CData"):
                    continue
                buf.append(str(child))
            elif isinstance(child, Tag):
                name = child.name
                if name in _SKIP:
                    continue
                if name == "img":
                    flush()
                    asset = _asset_name(child.get("src") or "")
                    if asset:
                        out.append(f"[asset: {asset}]")
                elif name in ("h1", "h2") and title is None:
                    flush()
                    text = " ".join(child.get_text(" ").split())
                    if text:
                        title = text
                elif name in _BLOCKS:
                    flush()
                    walk(child)
                    flush()
                else:
                    walk(child)

    walk(soup)
    flush()
    return title, out


class HtmlLoader:
    name = "html"
    extensions = (".html", ".htm", ".xhtml")

    def can_load(self, path: Path, head: bytes) -> bool:
        return path.suffix.lower() in self.extensions

    def load(self, path: Path) -> Book:
        data = path.read_bytes()
        title, paras = html_to_paragraphs(_decode(data))
        return Book(book_id(data), (Chapter(title, tuple(Paragraph(p) for p in paras)),))
