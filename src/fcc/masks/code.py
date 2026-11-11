from __future__ import annotations

from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, Segment, error_lines
from fcc.masks.palette import COMMENT, GUTTER, crop, highlight
from fcc.masks.snippets import BLOCKS, HEADERS, ITEMS
from fcc.text import ragged

LANGS = ("python", "typescript", "go", "rust", "java", "csharp")

_NUM_WIDTH = 5  # "%4d " gutter

# declarations a paragraph may use: its own plus filler, so neighbours never share one
_ITEMS_PER_BLOCK = 3

_FILES = {
    "python": "src/indexer.py",
    "typescript": "src/indexer.ts",
    "go": "indexer/indexer.go",
    "rust": "src/indexer.rs",
    "java": "src/main/java/Indexer.java",
    "csharp": "src/Indexer.cs",
}

# (prefix for each text line, opening lines, closing lines) of a doc comment
_FRAMES: dict[str, tuple[str, tuple[str, ...], tuple[str, ...]]] = {
    "python": ("# ", (), ()),
    "rust": ("/// ", (), ()),
    "go": ("// ", (), ()),
    "typescript": (" * ", ("/**",), (" */",)),
    "java": ("     * ", ("    /**",), ("     */",)),
    "csharp": ("    /// ", (), ()),
}


class CodeMask:
    """A documented source file: paragraphs are doc comments, filler is real code."""

    name = "code"

    def __init__(self, lang: str) -> None:
        if lang not in LANGS:
            raise ValueError(f"unknown language: {lang!r}")
        self.lang = lang
        # number for the next filler line: continues the last rendered block
        self._next_no = 1
        self._stream: list[tuple[Segment, ...]] = []  # pending lines of noise()
        # where in the corpus the next filler comes from: walking through it keeps
        # the same definition from turning up twice on one screen
        self._cursor = 0

    def _line(
        self, n: int, segments: tuple[Segment, ...], width: int, pos: Pos | None = None
    ) -> Line:
        gutter: Segment = (f"{n:>4} ", GUTTER)
        return Line(crop((gutter, *segments), max(1, width)), pos is not None, pos)

    def _numbered(self, rows: list[tuple[Segment, ...]], width: int) -> list[Line]:
        out = [self._line(self._next_no + i, segs, width) for i, segs in enumerate(rows)]
        self._next_no += len(rows)
        return out

    def header(self, rng: Random, width: int) -> list[Line]:
        rows = highlight(self.lang, HEADERS[self.lang])
        return [self._line(i, segs, width) for i, segs in enumerate(rows, 1)]

    def noise(self, rng: Random, width: int) -> Line:
        """Next line of an endless stream of complete definitions."""
        if not self._stream:
            blocks = BLOCKS[self.lang]
            self._stream = [(), *highlight(self.lang, blocks[self._cursor % len(blocks)])]
            self._cursor += 1
        return self._numbered([self._stream.pop(0)], width)[0]

    def noise_block(self, count: int, rng: Random, width: int) -> list[Line]:
        """Exactly `count` filler lines made of whole definitions, never a cut-off one."""
        blocks: list[tuple[tuple[Segment, ...], ...]] = []
        left = count
        # each paragraph draws from its own third of the corpus (the first three
        # blocks are short ones, one per third), so three paragraphs in a row
        # never show the same definition
        pool = [highlight(self.lang, b) for b in BLOCKS[self.lang]][self._cursor % 3 :: 3]
        turn = self._cursor // 3 % len(pool)
        pool = pool[turn:] + pool[:turn]
        while True:
            fits = [b for b in pool if len(b) + 1 <= left and b not in blocks]
            if not fits:
                break
            blocks.append(fits[0])
            left -= len(blocks[-1]) + 1
        items = ITEMS[self.lang]
        rows = [
            highlight(self.lang, items[(self._cursor * _ITEMS_PER_BLOCK + 1 + i) % len(items)])[0]
            for i in range(min(left, _ITEMS_PER_BLOCK - 1))
        ]
        rows += [()] * (left - len(rows))
        for block in blocks:
            rows += [(), *block]
        return self._numbered(rows, width)

    def line_count(self, para: Paragraph, width: int) -> int:
        prefix, opening, closing = _FRAMES[self.lang]
        body_width = max(1, width - _NUM_WIDTH - len(prefix))
        return len(ragged(para.text, body_width)) + len(opening) + len(closing) + 2

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]:
        prefix, opening, closing = _FRAMES[self.lang]
        body_width = max(1, width - _NUM_WIDTH - len(prefix))
        n = line_no_start
        out: list[Line] = []

        def emit(segments: tuple[Segment, ...], book: bool = False) -> None:
            nonlocal n
            out.append(self._line(n, segments, width, pos if book else None))
            n += 1

        emit(())
        for text in opening:
            emit(((text, COMMENT),))
        for text in ragged(para.text, body_width):
            emit(((prefix, COMMENT), (text, f"book {COMMENT}")), book=True)
        for text in closing:
            emit(((text, COMMENT),))
        self._cursor = pos[0] * 5 + pos[1]
        items = ITEMS[self.lang]
        emit(highlight(self.lang, items[self._cursor * _ITEMS_PER_BLOCK % len(items)])[0])
        self._next_no = n
        return out

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        """An editor status line: file, position, how far down."""
        pct = 0 if total <= 0 else min(100, max(0, done * 100 // total))
        return f" {_FILES[self.lang]}  {done}/{total}  {pct}%"
