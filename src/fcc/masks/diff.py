from __future__ import annotations

from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, error_lines, fit_lines, plain_line, progress_bar_label
from fcc.text import wrap

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
        return fit_lines(
            [
                plain_line(f"diff --git a/{path} b/{path}", "bold"),
                plain_line("index 3f2a1c4..9b7e0d2 100644", "dim"),
                plain_line(f"--- a/{path}", "bold"),
                plain_line(f"+++ b/{path}", "bold"),
            ],
            width,
        )

    def noise(self, rng: Random, width: int) -> Line:
        if rng.random() < 0.6:
            text, style = " " + rng.choice(_CONTEXT), ""
        else:
            text, style = "- " + rng.choice(_REMOVED), "red"
        return plain_line(text[: max(1, width)], style)

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]:
        texts = wrap(para.text, max(1, width - 2))
        start = 100 + line_no_start
        hunk = f"@@ -{start},7 +{start},{7 + len(texts)} @@ def handler():"
        out = [plain_line(hunk[: max(1, width)], "cyan")]
        out.append(self.noise(rng, width))
        out += [Line((("+ ", "green"), (t, "book green")), True, pos) for t in texts]
        out.append(self.noise(rng, width))
        return out

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        return progress_bar_label(done, total, label="Diffing")
