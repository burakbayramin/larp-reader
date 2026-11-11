from __future__ import annotations

from datetime import datetime, timedelta
from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, chrome, error_lines
from fcc.masks.palette import DIM, FG, GREEN, RED, YELLOW
from fcc.text import ragged

_EPOCH = datetime(2026, 11, 11, 18, 0, 0)
_AUTHORS = (
    ("Deniz Aksoy", "deniz.aksoy@example.com"),
    ("Mert Kaya", "mert@example.com"),
    ("Elif Demir", "elif.demir@example.com"),
    ("Alex Novak", "anovak@example.com"),
    ("Sam Patel", "sam.patel@example.com"),
)
_FILES = (
    "src/api/handlers.py",
    "src/api/routes.py",
    "src/core/cache.py",
    "src/core/scheduler.py",
    "src/db/migrations/0042_add_index.py",
    "src/db/models.py",
    "tests/test_handlers.py",
    "tests/test_scheduler.py",
    "pyproject.toml",
    "README.md",
    "docs/architecture.md",
    ".github/workflows/ci.yml",
)
_INDENT = 4
_HEADER_LINES = 4  # commit, Author, Date, blank
_STAT_WIDTH = 34


def _indent(width: int) -> int:
    return min(_INDENT, max(0, width // 8))


def _stat(rng: Random, width: int) -> Line:
    """One `git log --stat` file line: ` path | 12 +++++---`."""
    path = rng.choice(_FILES)
    adds, dels = rng.randint(0, 14), rng.randint(0, 9)
    adds += adds + dels == 0
    name = f" {path}"
    name = name[:_STAT_WIDTH].ljust(_STAT_WIDTH) if width >= 60 else name
    return chrome(
        width,
        (f"{name} | {adds + dels:>3} ", FG),
        ("+" * adds, GREEN),
        ("-" * dels, RED),
    )


class GitMask:
    """`git log --stat`: every paragraph is a commit message."""

    name = "git"

    def __init__(self, lang: str = "python") -> None:
        self.lang = lang

    def header(self, rng: Random, width: int) -> list[Line]:
        return [chrome(width, ("git log --stat --no-merges", DIM))]

    def noise(self, rng: Random, width: int) -> Line:
        return _stat(rng, width)

    def line_count(self, para: Paragraph, width: int) -> int:
        body = max(1, width - _indent(width))
        return _HEADER_LINES + len(ragged(para.text, body)) + 1

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]:
        sha = f"{rng.getrandbits(160):040x}"
        author, mail = rng.choice(_AUTHORS)
        # newest first, like the real thing: later text is older history
        when = _EPOCH - timedelta(minutes=line_no_start * 7 + rng.randint(0, 6))
        date = when.strftime("%a %b %d %H:%M:%S %Y") + " +0300"
        out = [
            chrome(width, (f"commit {sha}", YELLOW)),
            chrome(width, (f"Author: {author} <{mail}>", FG)),
            chrome(width, (f"Date:   {date}", FG)),
            chrome(width, ("", FG)),
        ]
        ind = _indent(width)
        for text in ragged(para.text, max(1, width - ind)):
            out.append(Line(((" " * ind, ""), (text, f"book {FG}")), True, pos))
        out.append(chrome(width, ("", FG)))
        return out

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        pct = 0 if total <= 0 else min(100, done * 100 // total)
        return f"Resolving deltas: {pct:3d}% ({done}/{total})"
