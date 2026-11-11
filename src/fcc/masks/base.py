from __future__ import annotations

from dataclasses import dataclass
from random import Random
from typing import Protocol

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.text import wrap

Segment = tuple[str, str]


@dataclass(frozen=True)
class Line:
    segments: tuple[Segment, ...]
    is_book: bool
    pos: Pos | None = None

    @property
    def plain(self) -> str:
        return "".join(text for text, _style in self.segments)


class Mask(Protocol):
    name: str

    def header(self, rng: Random, width: int) -> list[Line]: ...

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]: ...

    def noise(self, rng: Random, width: int) -> Line: ...

    def error(self, err: FccError, width: int) -> list[Line]: ...

    def progress_label(self, done: int, total: int, width: int) -> str: ...


def plain_line(text: str, style: str = "") -> Line:
    return Line(((text, style),), False)


def fit_lines(lines: list[Line], width: int) -> list[Line]:
    """Crop single-segment chrome lines to `width` columns."""
    return [plain_line(l.plain[: max(1, width)], l.segments[0][1] if l.segments else "") for l in lines]


def error_lines(err: FccError, width: int) -> list[Line]:
    lines = [plain_line(f"error: {err.title}"[: max(1, width)], "bold red")]
    lines += [plain_line(t, "red") for t in wrap(str(err), width)]
    return lines


def progress_bar_label(done: int, total: int, cells: int = 10, label: str = "Indexing") -> str:
    filled = 0 if total <= 0 else min(cells, max(0, done * cells // total))
    if filled >= cells:
        bar = "=" * cells
    else:
        bar = "=" * filled + ">" + " " * (cells - filled - 1)
    return f"{label} [{bar}] {done}/{total}"
