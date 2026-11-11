from __future__ import annotations

from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, chrome, error_lines
from fcc.masks.palette import DIM, FG, GREEN, RED
from fcc.text import ragged

_FILES = ("parser", "session", "render", "config", "index", "format", "cache")
_CASES = ("parses_chunk", "handles_empty", "rejects_bad_input", "roundtrip", "caches_result", "wraps_lines")


class TestMask:
    __test__ = False  # not a pytest test class
    name = "test"

    def __init__(self, lang: str = "python") -> None:
        self.lang = lang

    def header(self, rng: Random, width: int) -> list[Line]:
        # pytest centres its banner on the terminal width
        return [
            chrome(width, (" test session starts ".center(width, "="), f"bold {FG}")),
            chrome(width, ("platform linux -- Python 3.12.1, pytest-8.1.1, pluggy-1.4.0", FG)),
            chrome(width, ("rootdir: /home/dev/project", FG)),
            chrome(width, ("collected 380 items", f"bold {FG}")),
        ]

    def noise(self, rng: Random, width: int) -> Line:
        name = f"tests/test_{rng.choice(_FILES)}.py::test_{rng.choice(_CASES)}"
        return chrome(
            width, (f"{name} ", FG), ("PASSED", GREEN), (f" [{rng.randint(1, 100):3d}%]", DIM)
        )

    def line_count(self, para: Paragraph, width: int) -> int:
        return len(ragged(para.text, max(1, width - 4))) + 5

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
        fail = f"bold {RED}"
        out = [
            self.noise(rng, width),
            chrome(width, (f"_____ test_{case} _____", fail)),
            chrome(width, ("E   ", fail), ("AssertionError:", fail)),
        ]
        for t in ragged(para.text, max(1, width - 4)):
            out.append(Line((("E   ", fail), (t, f"book {FG}")), True, pos))
        out.append(
            chrome(width, (f"tests/test_{fname}.py", fail), (f":{line_no_start}: AssertionError", FG))
        )
        out.append(self.noise(rng, width))
        return out

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        pct = 0 if total <= 0 else min(100, max(0, done * 100 // total))
        return f"[{pct:3d}%] collected {total} items"
