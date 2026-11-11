from __future__ import annotations

from random import Random

from fcc.book import Book, Pos
from fcc.masks.base import Line, Mask
from fcc.noise import noise_count, noise_lines


class Session:
    """UI-independent reading engine: a position in a book plus render settings.

    `pos` is the paragraph holding the top line of the manual-mode view and
    `line_offset` is that line's index within `paragraph_lines(pos)`.  Setting
    `pos` puts the view at the top of that paragraph.
    """

    def __init__(self, book: Book, mask: Mask, noise: int, width: int, pos: Pos = (0, 0)):
        self.book = book
        self.mask = mask
        self.noise = noise
        self.width = width
        self.pos = pos

    @property
    def pos(self) -> Pos:
        return self._pos

    @pos.setter
    def pos(self, pos: Pos) -> None:
        self._pos = pos
        self.line_offset = 0

    def _seed(self, pos: Pos) -> str:
        return f"{self.book.id}:{pos}:{self.mask.name}:{self.noise}"

    def _noise_count(self, pos: Pos) -> int:
        # its own generator, so the count is known without rendering the paragraph
        return noise_count(self.noise, Random(self._seed(pos) + ":filler"))

    def _render(self, pos: Pos, start: int) -> list[Line]:
        rng = Random(self._seed(pos))
        lines = self.mask.render(self.book.paragraph(pos), pos, self.width, rng, start)
        count = self._noise_count(pos)
        return lines + noise_lines(self.mask, self.noise, rng, self.width, count)

    def _line_count(self, pos: Pos) -> int:
        return self.mask.line_count(self.book.paragraph(pos), self.width) + self._noise_count(pos)

    def _line_no_start(self, pos: Pos) -> int:
        """Number of the paragraph's first line, counted from the top of the text.

        Lines are numbered continuously like a real file, so a paragraph's start
        depends on every paragraph before it; the running totals are kept until
        the mask, noise level or width changes.
        """
        settings = (self.mask, self.noise, self.width)
        known = getattr(self, "_numbering", None)
        if known is None or known[0] is not self.mask or known[1:] != settings[1:]:
            self._numbering = settings
            first = next((c, 0) for c, ch in enumerate(self.book.chapters) if ch.paragraphs)
            header = self.mask.header(Random(self.book.id), self.width)
            self._order: list[Pos] = [first]
            self._starts: list[int] = [len(header) + 1]
        index = self.book.index_of(pos)
        while len(self._starts) <= index:
            last = self._order[-1]
            self._starts.append(self._starts[-1] + self._line_count(last))
            self._order.append(self.book.next_pos(last))
        return self._starts[index]

    def paragraph_lines(self, pos: Pos) -> list[Line]:
        return self._render(pos, self._line_no_start(pos))

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

    def window(self, rows: int) -> list[Line]:
        """Up to `rows` rendered lines starting at the view's top line (pos, line_offset)."""
        out: list[Line] = []
        cur: Pos | None = self.pos
        skip = min(self.line_offset, len(self.paragraph_lines(self.pos)) - 1)
        while cur is not None and len(out) < rows:
            out += self.paragraph_lines(cur)[skip:]
            skip = 0
            cur = self.book.next_pos(cur)
        return out[:rows]

    def scroll(self, n: int) -> bool:
        """Move the view's top line by `n` rendered lines, across paragraphs.

        Stops at the first line of the text; a forward move whose target line
        does not exist is not made.  Returns whether the view moved.
        """
        pos, off = self.pos, min(self.line_offset, len(self.paragraph_lines(self.pos)) - 1)
        off += n
        while off < 0:
            prev = self.book.prev_pos(pos)
            if prev is None:
                off = 0
                break
            pos = prev
            off += len(self.paragraph_lines(pos))
        while off >= (count := len(self.paragraph_lines(pos))):
            nxt = self.book.next_pos(pos)
            if nxt is None:
                return False
            off -= count
            pos = nxt
        moved = (pos, off) != (self.pos, self.line_offset)
        self.pos = pos
        self.line_offset = off
        return moved

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
