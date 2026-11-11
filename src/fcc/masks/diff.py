from __future__ import annotations

from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, chrome, error_lines, progress_bar_label
from fcc.masks.palette import CYAN, DIM, FG, GREEN, RED, highlight
from fcc.text import ragged

_CONTEXT = [
    "    ctx = build_context(request)",
    "    if not payload:",
    "        return None",
    "    for item in queue:",
    "    return response",
    "    logger.debug(\"handled\")",
]
_REMOVED = [
    "    result = old_handler(request)",
    "    cache.clear()",
    "    timeout = 30",
    "    raise NotImplementedError",
]


class DiffMask:
    name = "diff"

    def __init__(self, lang: str = "python") -> None:
        self.lang = lang

    def header(self, rng: Random, width: int) -> list[Line]:
        # a shorter path in narrow panes, rather than a cut-off `diff --git` line
        full = "diff --git a/src/handler.py b/src/handler.py"
        path = "src/handler.py" if len(full) <= width else "handler.py"
        head = f"bold {FG}"
        return [
            chrome(width, (f"diff --git a/{path} b/{path}", head)),
            chrome(width, ("index 3f2a1c4..9b7e0d2 100644", DIM)),
            chrome(width, (f"--- a/{path}", head)),
            chrome(width, (f"+++ b/{path}", head)),
        ]

    def noise(self, rng: Random, width: int) -> Line:
        if rng.random() < 0.6:
            return chrome(width, (" ", FG), *highlight("python", rng.choice(_CONTEXT))[0])
        return chrome(width, ("-", RED), (" " + rng.choice(_REMOVED), RED))

    def line_count(self, para: Paragraph, width: int) -> int:
        return len(ragged(para.text, max(1, width - 2))) + 3

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]:
        texts = ragged(para.text, max(1, width - 2))
        start = 100 + line_no_start
        out = [
            chrome(
                width,
                (f"@@ -{start},7 +{start},{7 + len(texts)} @@", CYAN),
                (" def handler():", FG),
            )
        ]
        out.append(self.noise(rng, width))
        out += [Line((("+ ", GREEN), (t, f"book {GREEN}")), True, pos) for t in texts]
        out.append(self.noise(rng, width))
        return out

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        return progress_bar_label(done, total, label="Diffing")
