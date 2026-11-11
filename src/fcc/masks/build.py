from __future__ import annotations

from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, chrome, error_lines, progress_bar_label
from fcc.masks.palette import BLUE, FG, GREEN, YELLOW
from fcc.text import wrap

_CRATES = ("serde", "tokio", "regex", "anyhow", "hyper", "clap", "memchr", "once_cell", "bytes", "itoa")
_FILES = ("parser", "index", "render", "config", "cache")
_VARS = ("chunk", "offset", "buf", "state", "count")
_GUTTER = "   |  "


class BuildMask:
    name = "build"

    def __init__(self, lang: str = "rust") -> None:
        self.lang = lang

    def header(self, rng: Random, width: int) -> list[Line]:
        return [chrome(width, ("    Updating", f"bold {GREEN}"), (" crates.io index", FG))]

    def noise(self, rng: Random, width: int) -> Line:
        ver = f"{rng.randint(0, 2)}.{rng.randint(0, 20)}.{rng.randint(0, 9)}"
        return chrome(width, ("   Compiling", f"bold {GREEN}"), (f" {rng.choice(_CRATES)} v{ver}", FG))

    def line_count(self, para: Paragraph, width: int) -> int:
        return len(wrap(para.text, max(1, width - len(_GUTTER)))) + 6

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]:
        rail = f"bold {BLUE}"
        where = f"src/{rng.choice(_FILES)}.rs:{line_no_start}:{rng.randint(5, 20)}"
        out = [
            self.noise(rng, width),
            chrome(
                width,
                ("warning", f"bold {YELLOW}"),
                (f": unused variable: `{rng.choice(_VARS)}`", f"bold {FG}"),
            ),
            chrome(width, ("  --> ", rail), (where, FG)),
            chrome(width, ("   |", rail)),
        ]
        for t in wrap(para.text, max(1, width - len(_GUTTER))):
            out.append(Line(((_GUTTER, rail), (t, f"book {FG}")), True, pos))
        out.append(chrome(width, ("   |", rail)))
        out.append(self.noise(rng, width))
        return out

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        return progress_bar_label(done, total, label="Building")
