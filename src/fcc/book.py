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


def book_id(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()[:16]
