from __future__ import annotations

from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, error_lines, plain_line
from fcc.text import wrap

_FILES = ("parser", "session", "render", "config", "index", "format", "cache")
_CASES = ("parses_chunk", "handles_empty", "rejects_bad_input", "roundtrip", "caches_result", "wraps_lines")


class TestMask:
    __test__ = False  # not a pytest test class
    name = "test"

    def __init__(self, lang: str = "python") -> None:
        self.lang = lang

    def header(self, rng: Random) -> list[Line]:
        return [
            plain_line("=" * 29 + " test session starts " + "=" * 30, "bold"),
            plain_line("platform linux -- Python 3.12.1, pytest-8.1.1, pluggy-1.4.0", ""),
            plain_line("rootdir: /home/dev/project", ""),
            plain_line("collected 380 items", ""),
        ]

    def noise(self, rng: Random, width: int) -> Line:
        name = f"tests/test_{rng.choice(_FILES)}.py::test_{rng.choice(_CASES)}"
        text = f"{name} PASSED [{rng.randint(1, 100):3d}%]"
        return plain_line(text[: max(1, width)], "green")

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]:
        case = rng.choice(_CASES)
        fname = rng.choice(_FILES)
        out = [
            self.noise(rng, width),
            plain_line(f"_____ test_{case} _____"[: max(1, width)], "bold red"),
            plain_line("E   AssertionError:"[: max(1, width)], "red"),
        ]
        for t in wrap(para.text, max(1, width - 4)):
            out.append(Line((("E   ", "red"), (t, "book red")), True, pos))
        loc = f"tests/test_{fname}.py:{line_no_start}: AssertionError"
        out.append(plain_line(loc[: max(1, width)], "red"))
        out.append(self.noise(rng, width))
        return out

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        pct = 0 if total <= 0 else min(100, max(0, done * 100 // total))
        return f"[{pct:3d}%] collected {total} items"
