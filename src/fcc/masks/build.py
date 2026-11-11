from __future__ import annotations

from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, error_lines, fit_lines, plain_line, progress_bar_label
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
        return fit_lines([plain_line("    Updating crates.io index", "bold green")], width)

    def noise(self, rng: Random, width: int) -> Line:
        ver = f"{rng.randint(0, 2)}.{rng.randint(0, 20)}.{rng.randint(0, 9)}"
        text = f"   Compiling {rng.choice(_CRATES)} v{ver}"
        return plain_line(text[: max(1, width)], "green")

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]:
        def fit(t: str) -> str:
            return t[: max(1, width)]

        out = [
            self.noise(rng, width),
            plain_line(fit(f"warning: unused variable: `{rng.choice(_VARS)}`"), "bold yellow"),
            plain_line(fit(f"  --> src/{rng.choice(_FILES)}.rs:{line_no_start}:{rng.randint(5, 20)}"), "cyan"),
            plain_line(fit("   |"), "cyan"),
        ]
        for t in wrap(para.text, max(1, width - len(_GUTTER))):
            out.append(Line(((_GUTTER, "cyan"), (t, "book yellow")), True, pos))
        out.append(plain_line(fit("   |"), "cyan"))
        out.append(self.noise(rng, width))
        return out

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        return progress_bar_label(done, total, label="Building")
