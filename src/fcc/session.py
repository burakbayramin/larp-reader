from __future__ import annotations

from random import Random

from fcc.book import Book, Pos
from fcc.masks.base import Line, Mask
from fcc.noise import noise_lines

_LINE_NO_STRIDE = 40


class Session:
    """UI-independent reading engine: a position in a book plus render settings."""

    def __init__(self, book: Book, mask: Mask, noise: int, width: int, pos: Pos = (0, 0)):
        self.book = book
        self.mask = mask
        self.noise = noise
        self.width = width
        self.pos = pos

    def paragraph_lines(self, pos: Pos) -> list[Line]:
        rng = Random(f"{self.book.id}:{pos}:{self.mask.name}:{self.noise}")
        start = self.book.index_of(pos) * _LINE_NO_STRIDE + 1
        lines = self.mask.render(self.book.paragraph(pos), pos, self.width, rng, start)
        return lines + noise_lines(self.mask, self.noise, rng, self.width)

    def lines_from(self, pos: Pos, max_book_lines: int) -> list[Line]:
        out: list[Line] = []
        book_lines = 0
        cur: Pos | None = pos
        while cur is not None and book_lines < max_book_lines:
            para = self.paragraph_lines(cur)
            out += para
            book_lines += sum(ln.is_book for ln in para)
            cur = self.book.next_pos(cur)
        return out

    def advance(self, n: int = 1) -> bool:
        return self._move(n, self.book.next_pos)

    def back(self, n: int = 1) -> bool:
        return self._move(n, self.book.prev_pos)

    def _move(self, n: int, step) -> bool:
        cur = self.pos
        for _ in range(n):
            nxt = step(cur)
            if nxt is None:
                self.pos = cur
                return False
            cur = nxt
        self.pos = cur
        return True

    def progress(self) -> tuple[int, int]:
        return (self.book.index_of(self.pos) + 1, self.book.paragraph_count())
