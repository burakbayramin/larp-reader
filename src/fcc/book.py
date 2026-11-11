from __future__ import annotations

import hashlib
from dataclasses import dataclass

Pos = tuple[int, int]


@dataclass(frozen=True)
class Paragraph:
    text: str


@dataclass(frozen=True)
class Chapter:
    title: str | None
    paragraphs: tuple[Paragraph, ...]


@dataclass(frozen=True)
class Book:
    id: str
    chapters: tuple[Chapter, ...]

    def paragraph_count(self) -> int:
        return sum(len(c.paragraphs) for c in self.chapters)

    def paragraph(self, pos: Pos) -> Paragraph:
        return self.chapters[pos[0]].paragraphs[pos[1]]

    def next_pos(self, pos: Pos) -> Pos | None:
        ci, pi = pos
        if pi + 1 < len(self.chapters[ci].paragraphs):
            return (ci, pi + 1)
        for nci in range(ci + 1, len(self.chapters)):
            if self.chapters[nci].paragraphs:
                return (nci, 0)
        return None

    def prev_pos(self, pos: Pos) -> Pos | None:
        ci, pi = pos
        if pi > 0:
            return (ci, pi - 1)
        for pci in range(ci - 1, -1, -1):
            n = len(self.chapters[pci].paragraphs)
            if n:
                return (pci, n - 1)
        return None

    def index_of(self, pos: Pos) -> int:
        ci, pi = pos
        return sum(len(c.paragraphs) for c in self.chapters[:ci]) + pi

    # -- chapters ---------------------------------------------------------

    def chapter_list(self) -> list[tuple[int, str | None]]:
        """(chapter index, title) of every chapter that has text."""
        return [(ci, c.title) for ci, c in enumerate(self.chapters) if c.paragraphs]

    def next_chapter(self, pos: Pos) -> Pos | None:
        for ci in range(pos[0] + 1, len(self.chapters)):
            if self.chapters[ci].paragraphs:
                return (ci, 0)
        return None

    def prev_chapter(self, pos: Pos) -> Pos | None:
        """The start of this chapter, or of the previous one when already there."""
        if pos[1] > 0:
            return (pos[0], 0)
        for ci in range(pos[0] - 1, -1, -1):
            if self.chapters[ci].paragraphs:
                return (ci, 0)
        return None

    # -- search -----------------------------------------------------------

    def find(self, query: str, start: Pos, backward: bool = False) -> tuple[Pos, bool] | None:
        """First paragraph from `start` (inclusive) on containing `query`.

        Searches toward the end (or the beginning when `backward`) and wraps
        around once.  Returns the paragraph and whether the search wrapped.
        """
        needle = fold(query.strip())
        if not needle:
            return None
        order = [(ci, pi) for ci, c in enumerate(self.chapters) for pi in range(len(c.paragraphs))]
        if not order:
            return None
        here = order.index(start) if start in order else 0
        n = len(order)
        for k in range(n):
            i = here - k if backward else here + k
            wrapped = i < 0 or i >= n
            pos = order[i % n]
            if needle in fold(self.paragraph(pos).text):
                return pos, wrapped
        return None


def fold(text: str) -> str:
    """Case-folded text of the same length; Turkish dotted/dotless i count as plain i."""
    out = []
    for ch in text:
        if ch in "İIı":
            out.append("i")
            continue
        low = ch.lower()
        out.append(low if len(low) == 1 else ch)
    return "".join(out)


def match_spans(text: str, query: str) -> list[tuple[int, int]]:
    """Non-overlapping (start, end) ranges where `query` occurs in `text`, ignoring case."""
    needle = fold(query.strip())
    if not needle:
        return []
    hay = fold(text)
    spans: list[tuple[int, int]] = []
    i = hay.find(needle)
    while i >= 0:
        spans.append((i, i + len(needle)))
        i = hay.find(needle, i + len(needle))
    return spans


def book_id(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()[:16]
