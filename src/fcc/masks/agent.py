"""A coding-agent session: tool calls, coloured diffs, command output and narration.

Most paragraphs appear as the added lines of an edit, the rest as the agent's
own remarks.  Filler comes from the shuffled pools in `codegen`, read by an
index tied to the paragraph's line number: neighbouring paragraphs read
different parts of the pools, and redrawing a paragraph gives the same lines.
"""
from __future__ import annotations

import os
import zlib
from dataclasses import dataclass
from functools import lru_cache
from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, Segment, chrome, crop, error_lines
from fcc.masks.codegen import COMMANDS, FILES, SIGNATURES, STATEMENTS, TASKS
from fcc.masks.palette import DIM, FG, GREEN, RED, highlight
from fcc.text import ragged

BULLET = "●" if os.name == "nt" else "⏺"
ELBOW = "  ⎿  "  # first line of a tool result
CONT = "     "  # its following lines
ADD_BG = "on #1f3d27"
DEL_BG = "on #4b1f22"
_THINKING = "✻ Thinking…"
_GUTTER = 8  # "%7d " before the +/- sign
_SIGN = 2  # sign and a space
_BODY_INDENT = 8  # statements sit inside a method
_PER_SLOT = 8  # statements one slot may use
_STREAM_BASE = 10_000_000  # the hidden stream reads a part of the pools the text never reaches


@dataclass(frozen=True)
class Layout:
    kind: str  # "edit" (text as added lines) or "say" (text as narration)
    removed: int  # lines the edit takes out
    before: int  # unchanged lines shown above the change


def layout(text: str) -> Layout:
    """How a paragraph is laid out; from its text alone, so it can be counted without drawing."""
    h = zlib.crc32(text.encode("utf-8"))
    kind = "say" if h % 10 < 3 else "edit"
    return Layout(kind, (h >> 4) % 4, 1 + (h >> 8) % 2)


@lru_cache(maxsize=4096)
def _code(line: str) -> tuple[Segment, ...]:
    return highlight("python", line)[0] if line.strip() else ()


def _plural(n: int, word: str) -> str:
    return f"{n} {word}" if n == 1 else f"{n} {word}s"


def _indent(width: int) -> int:
    return min(_BODY_INDENT, max(0, (width - _GUTTER - _SIGN) // 10))


def _body_width(width: int) -> int:
    return max(1, width - _GUTTER - _SIGN - _indent(width))


def _pad(segments: tuple[Segment, ...], width: int, bg: str) -> tuple[Segment, ...]:
    used = sum(len(t) for t, _ in segments)
    return segments + ((" " * (width - used), bg),) if used < width else segments


# Every pool is read by a "slot": the line number where a paragraph or a filler
# unit starts.  Line numbers are unique along the text, so no two places read
# the same entry until a pool has been gone through.
def _stmt(slot: int, j: int) -> str:
    return STATEMENTS[slot * _PER_SLOT + j]


def _func_name(signature: str) -> str:
    return signature.split("(")[0].split()[-1]


class AgentMask:
    name = "agent"

    def __init__(self, lang: str = "python") -> None:
        self.lang = lang
        self._next = 0  # slot (line number) where the filler after the last paragraph starts
        self._stream: list[Line] = []  # pending lines of the hidden stream
        self._stream_n = 0
        self._last_unit = ""

    # -- building blocks ---------------------------------------------------

    def _tool(self, width: int, name: str, arg: str) -> Line:
        return chrome(width, (f"{BULLET} ", GREEN), (name, f"bold {FG}"), (f"({arg})", FG))

    def _result(self, width: int, first: bool, *segments: Segment) -> Line:
        return chrome(width, (ELBOW if first else CONT, DIM), *segments)

    def _context(self, width: int, n: int, code: str) -> Line:
        return chrome(width, (f"{n:>7} ", DIM), ("  ", FG), *_code(code))

    def _removed(self, width: int, n: int, code: str) -> Line:
        segs = crop(((f"{n:>7} ", DIM), ("- ", f"{RED} {DEL_BG}"), (code, f"{FG} {DEL_BG}")), width)
        return Line(_pad(segs, width, DEL_BG), False)

    def _added(self, width: int, n: int, code: str) -> Line:
        segs = crop(((f"{n:>7} ", DIM), ("+ ", f"{GREEN} {ADD_BG}"), (code, f"{FG} {ADD_BG}")), width)
        return Line(_pad(segs, width, ADD_BG), False)

    def _edit_head(self, width: int, path: str, adds: int, rems: int) -> list[Line]:
        return [
            self._tool(width, "Update", path),
            self._result(
                width, True, ("Updated ", FG), (path, f"bold {FG}"),
                (f" with {_plural(adds, 'addition')} and {_plural(rems, 'removal')}", FG),
            ),
        ]

    # -- the text ------------------------------------------------------------

    def header(self, rng: Random, width: int) -> list[Line]:
        return [chrome(width, ("> refactor the cache layer and add tests", DIM)), chrome(width, ("", FG))]

    def line_count(self, para: Paragraph, width: int) -> int:
        lay = layout(para.text)
        if lay.kind == "say":
            return len(ragged(para.text, max(1, width - 2))) + 1
        return 2 + lay.before + lay.removed + len(ragged(para.text, _body_width(width))) + 1 + 1

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]:
        lay = layout(para.text)
        slot = line_no_start
        if lay.kind == "say":
            out = []
            for k, text in enumerate(ragged(para.text, max(1, width - 2))):
                lead: Segment = (f"{BULLET} ", FG) if k == 0 else ("  ", FG)
                out.append(Line((lead, (text, f"book {FG}")), True, pos))
            out.append(chrome(width, ("", FG)))
            self._next = slot + len(out)
            return out

        texts = ragged(para.text, _body_width(width))
        path = FILES[slot]
        first = 20 + (line_no_start * 13) % 380
        out = self._edit_head(width, path, len(texts), lay.removed)
        n = first
        out.append(self._context(width, n, "    " + SIGNATURES[slot]))
        n += 1
        for k in range(1, lay.before):
            out.append(self._context(width, n, " " * _BODY_INDENT + _stmt(slot, k)))
            n += 1
        for k in range(lay.removed):
            out.append(self._removed(width, n + k, " " * _BODY_INDENT + _stmt(slot, 2 + k)))
        indent = " " * _indent(width)
        for k, text in enumerate(texts):
            segs = (
                (f"{n + k:>7} ", DIM),
                ("+ ", f"{GREEN} {ADD_BG}"),
                (indent, ADD_BG),
                (text, f"book {FG} {ADD_BG}"),
            )
            out.append(Line(_pad(segs, width, ADD_BG), True, pos))
        out.append(self._context(width, n + len(texts), " " * _BODY_INDENT + _stmt(slot, 7)))
        out.append(chrome(width, ("", FG)))
        self._next = slot + len(out)
        return out

    # -- filler --------------------------------------------------------------

    def _unit(self, kind: str, slot: int, rng: Random, width: int) -> list[Line]:
        """One whole tool call (with its result and a blank line after it)."""
        blank = chrome(width, ("", FG))
        if kind == "read":
            return [
                self._tool(width, "Read", FILES[slot]),
                self._result(width, True, ("Read ", FG), (f"{rng.randint(40, 480)}", f"bold {FG}"), (" lines", FG)),
                blank,
            ]
        if kind == "search":
            where = FILES[slot]  # a whole file, so the same call rarely comes back
            return [
                self._tool(width, "Search", f'pattern: "{_func_name(SIGNATURES[slot])}", path: "{where}"'),
                self._result(width, True, ("Found ", FG), (f"{rng.randint(2, 14)}", f"bold {FG}"), (" lines", FG)),
                blank,
            ]
        if kind == "bash":
            cmd = COMMANDS[slot]
            return [self._tool(width, "Bash", cmd), *self._output(cmd, slot, rng, width), blank]
        if kind == "todos":
            n = rng.randint(2, 3)
            rows = []
            for k in range(n):
                done = k < n - 1
                mark, style = ("☒ ", DIM) if done else ("☐ ", FG)
                rows.append(self._result(width, k == 0, (mark, style), (TASKS[slot * 3 + k], style)))
            return [chrome(width, (f"{BULLET} ", GREEN), ("Update Todos", f"bold {FG}")), *rows, blank]
        # a small edit with no text in it
        path = FILES[slot]
        n = 20 + slot * 7 % 300
        return [
            *self._edit_head(width, path, 1, 1),
            self._context(width, n, "    " + SIGNATURES[slot]),
            self._removed(width, n + 1, " " * _BODY_INDENT + _stmt(slot, 0)),
            self._added(width, n + 1, " " * _BODY_INDENT + _stmt(slot, 1)),
            blank,
        ]

    def _output(self, cmd: str, slot: int, rng: Random, width: int) -> list[Line]:
        if cmd.startswith(("pytest", "python -m pytest")):
            passed = rng.randint(3, 60)
            dots = "." * min(passed, 40)
            return [
                self._result(width, True, (dots, GREEN), (f"  [100%]", FG)),
                self._result(width, False, (f"{passed} passed", GREEN), (f" in {rng.randint(8, 300) / 100:.2f}s", FG)),
            ]
        if cmd.startswith("ruff"):
            return [self._result(width, True, ("All checks passed!", FG))]
        if cmd.startswith("mypy"):
            return [self._result(width, True, ("Success: no issues found in 1 source file", GREEN))]
        if cmd.startswith("rg"):
            return [
                self._result(width, k == 0, (f"{FILES[slot + k]}:{rng.randint(10, 400)}:", DIM), (SIGNATURES[slot + k].strip(), FG))
                for k in range(rng.randint(1, 2))
            ]
        if cmd.startswith("git log"):
            return [
                self._result(width, k == 0, (f"{rng.getrandbits(28):07x} ", "#d29922"), (TASKS[slot * 3 + k], FG))
                for k in range(rng.randint(1, 3))
            ]
        # git diff --stat
        rows = []
        k = rng.randint(1, 2)
        for j in range(k):
            a, d = rng.randint(1, 30), rng.randint(0, 12)
            rows.append(self._result(width, j == 0, (f" {FILES[slot + j]} | {a + d:>3} ", FG), ("+" * min(a, 12), GREEN), ("-" * min(d, 8), RED)))
        rows.append(self._result(width, False, (f" {k} files changed", FG)))
        return rows

    def _pick(self, kinds: list[str], rng: Random) -> str:
        choices = [k for k in kinds if k != self._last_unit] or kinds
        self._last_unit = rng.choice(choices)
        return self._last_unit

    def noise_block(self, count: int, rng: Random, width: int) -> list[Line]:
        """Exactly `count` lines of whole tool calls; leftover rows become a thinking line or a gap."""
        out: list[Line] = []
        self._last_unit = ""
        tries = 0
        while len(out) < count and tries < 20:
            tries += 1
            kind = self._pick(["read", "search", "bash", "todos", "edit"], rng)
            # a unit's slot is the line number it starts on: unique, like the text's own
            unit = self._unit(kind, self._next + len(out), rng, width)
            if len(unit) <= count - len(out):
                out += unit
        # rows no whole unit fits into go first, so they follow the blank line ending the
        # paragraph above instead of sitting right on top of the next one
        left = count - len(out)
        lead = [chrome(width, (_THINKING, DIM))] + [chrome(width, ("", FG))] * (left - 1) if left else []
        return lead + out

    def noise(self, rng: Random, width: int) -> Line:
        """Next line of an endless session, for the hidden screen."""
        if not self._stream:
            kind = self._pick(["read", "search", "bash", "todos", "edit", "edit"], rng)
            self._stream = self._unit(kind, _STREAM_BASE + self._stream_n, rng, width)
            if rng.random() < 0.15:
                self._stream.insert(0, chrome(width, (_THINKING, DIM)))
            self._stream_n += 1
        return self._stream.pop(0)

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        tokens = done * 37 / 10
        return f"✻ Working… ({done}/{total} · ↓ {tokens / 1000:.1f}k tokens)"
