from __future__ import annotations

import warnings
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

from ebooklib import ITEM_DOCUMENT, epub

from fcc.book import Book, Chapter, Paragraph, book_id
from fcc.errors import DrmProtected, UnsupportedFormat
from fcc.loaders.html import html_to_paragraphs

# Font obfuscation (IDPF / Adobe) is not DRM.
_FONT_ALGORITHMS = {
    "http://www.idpf.org/2008/embedding",
    "http://ns.adobe.com/pdf/enc#RC",
}


def _check_drm(path: Path) -> None:
    try:
        with zipfile.ZipFile(path) as z:
            raw = z.read("META-INF/encryption.xml")
    except KeyError:
        return
    except zipfile.BadZipFile as e:
        raise UnsupportedFormat(f"dosya açılamadı: {e}") from e
    try:
        root = ET.fromstring(raw)
    except ET.ParseError:
        raise DrmProtected("DRM korumalı, açılamaz") from None
    for el in root.iter():
        if el.tag.rsplit("}", 1)[-1] == "EncryptionMethod" and el.get("Algorithm") not in _FONT_ALGORITHMS:
            raise DrmProtected("DRM korumalı, açılamaz")


class EpubLoader:
    name = "epub"
    extensions = (".epub",)

    def can_load(self, path: Path, head: bytes) -> bool:
        return path.suffix.lower() in self.extensions

    def load(self, path: Path) -> Book:
        _check_drm(path)
        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                eb = epub.read_epub(str(path), {"ignore_ncx": True})
        except Exception as e:
            raise UnsupportedFormat(f"dosya açılamadı: {e}") from e
        chapters: list[Chapter] = []
        for entry in eb.spine:
            item_id = entry[0] if isinstance(entry, tuple) else entry
            item = eb.get_item_with_id(item_id)
            if item is None or item.get_type() != ITEM_DOCUMENT:
                continue
            title, paras = html_to_paragraphs(item.get_content().decode("utf-8", errors="replace"))
            if paras:
                chapters.append(Chapter(title, tuple(Paragraph(p) for p in paras)))
        return Book(book_id(path.read_bytes()), tuple(chapters))
