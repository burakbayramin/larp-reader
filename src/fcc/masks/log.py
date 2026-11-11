from __future__ import annotations

from datetime import datetime, timedelta
from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, Segment, chrome, error_lines, progress_bar_label
from fcc.masks.palette import BLUE, CYAN, DIM, FG, GREEN, YELLOW
from fcc.text import wrap

_EPOCH = datetime(2026, 11, 11, 9, 0, 0)
_BOOK_SVC = "svc.worker"
_SVC_WIDTH = 10
_SERVICES = ("svc.worker", "svc.api", "svc.cache", "svc.queue", "svc.auth")
_SHORT_LEVEL = {"INFO": "INF", "DEBUG": "DBG", "WARN": "WRN"}
_FULL_MIN_WIDTH = 100  # below this the ISO prefix leaves too little room for text
_COMPACT_MIN_WIDTH = 60
_MESSAGES = {
    "INFO": [
        "request completed in 12ms",
        "connection pool size=8 idle=5",
        "flushed 128 records to disk",
        "heartbeat ok",
        "job 4821 finished status=done",
    ],
    "DEBUG": [
        "cache lookup key=user:1042 hit=true",
        "retry budget remaining=3",
        "parsed payload bytes=2048",
        "scheduler tick queue_depth=0",
    ],
    "WARN": [
        "slow query detected took=840ms",
        "retrying upstream call attempt=2",
        "queue depth above soft limit",
    ],
}


def _stamp(t: datetime) -> str:
    return t.strftime("%Y-%m-%dT%H:%M:%S.") + f"{t.microsecond // 1000:03d}Z"


_LEVEL_STYLE = {"INFO": GREEN, "DEBUG": BLUE, "WARN": f"bold {YELLOW}"}
_MAX_STEP_MS = 700  # below the per-line budget of render(), so time never runs backwards
_LINE_MS = 731


def _prefix_segments(t: datetime, level: str, svc: str, width: int) -> tuple[Segment, ...]:
    """Log-line prefix sized to the pane: full ISO (~43 cols), compact (~26) or tiny (13)."""
    lvl = _LEVEL_STYLE[level]
    if width >= _FULL_MIN_WIDTH:
        return ((f"{_stamp(t)} ", DIM), (f"{level:<5} ", lvl), (f"{svc:<{_SVC_WIDTH}}  ", CYAN))
    clock = t.strftime("%H:%M:%S")
    if width >= _COMPACT_MIN_WIDTH:
        module = svc.removeprefix("svc.")
        return (
            (f"{clock}.{t.microsecond // 1000:03d} ", DIM),
            (f"{level:<5} ", lvl),
            (f"{module:<6} ", CYAN),
        )
    return ((f"{clock} ", DIM), (f"{_SHORT_LEVEL[level]} ", lvl))


def _prefix(t: datetime, level: str, svc: str, width: int) -> str:
    return "".join(text for text, _style in _prefix_segments(t, level, svc, width))


class LogMask:
    name = "log"

    def __init__(self, lang: str = "python") -> None:
        self.lang = lang
        self._t = _EPOCH  # time of the last line: filler continues from it

    def header(self, rng: Random, width: int) -> list[Line]:
        text = "-- Logs begin at Wed 2026-11-11 09:00:00 UTC --"
        if len(text) > width:
            text = "-- Logs begin at 2026-11-11 09:00:00 --"
        return [chrome(width, (text, DIM))]

    def _tick(self, rng: Random) -> datetime:
        self._t += timedelta(milliseconds=rng.randint(20, _MAX_STEP_MS))
        return self._t

    def _entry(self, rng: Random, t: datetime, width: int) -> Line:
        level = rng.choices(["INFO", "DEBUG", "WARN"], weights=[6, 2, 1])[0]
        prefix = _prefix_segments(t, level, rng.choice(_SERVICES), width)
        return chrome(width, *prefix, (rng.choice(_MESSAGES[level]), FG))

    def noise(self, rng: Random, width: int) -> Line:
        return self._entry(rng, self._tick(rng), width)

    def line_count(self, para: Paragraph, width: int) -> int:
        body = max(1, width - len(_prefix(_EPOCH, "INFO", _BOOK_SVC, width)))
        return len(wrap(para.text, body)) + 2

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]:
        self._t = _EPOCH + timedelta(milliseconds=line_no_start * _LINE_MS)
        out = [self._entry(rng, self._tick(rng), width)]
        body = max(1, width - len(_prefix(self._t, "INFO", _BOOK_SVC, width)))
        for text in wrap(para.text, body):
            prefix = _prefix_segments(self._tick(rng), "INFO", _BOOK_SVC, width)
            out.append(Line((*prefix, (text, f"book {FG}")), True, pos))
        out.append(self._entry(rng, self._tick(rng), width))
        return out

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        return progress_bar_label(done, total, label="Tailing")
