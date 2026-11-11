from __future__ import annotations

from datetime import datetime, timedelta
from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, error_lines, fit_lines, plain_line, progress_bar_label
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


def _prefix(t: datetime, level: str, svc: str, width: int) -> str:
    """Log-line prefix sized to the pane: full ISO (~43 cols), compact (~26) or tiny (13)."""
    if width >= _FULL_MIN_WIDTH:
        return f"{_stamp(t)} {level:<5} {svc:<{_SVC_WIDTH}}  "
    if width >= _COMPACT_MIN_WIDTH:
        module = svc.removeprefix("svc.")
        return f"{t.strftime('%H:%M:%S')}.{t.microsecond // 1000:03d} {level:<5} {module:<6} "
    return f"{t.strftime('%H:%M:%S')} {_SHORT_LEVEL[level]} "


class LogMask:
    name = "log"

    def __init__(self, lang: str = "python") -> None:
        self.lang = lang

    def header(self, rng: Random, width: int) -> list[Line]:
        text = "-- Logs begin at Wed 2026-11-11 09:00:00 UTC --"
        if len(text) > width:
            text = "-- Logs begin at 2026-11-11 09:00:00 --"
        return fit_lines([plain_line(text, "dim")], width)

    def _entry(self, rng: Random, t: datetime, width: int) -> Line:
        level = rng.choices(["INFO", "DEBUG", "WARN"], weights=[6, 2, 1])[0]
        text = _prefix(t, level, rng.choice(_SERVICES), width) + rng.choice(_MESSAGES[level])
        return plain_line(text[: max(1, width)], "yellow" if level == "WARN" else "")

    def noise(self, rng: Random, width: int) -> Line:
        t = _EPOCH + timedelta(milliseconds=rng.randint(0, 3_600_000))
        return self._entry(rng, t, width)

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]:
        t = _EPOCH + timedelta(milliseconds=line_no_start * 731)
        out: list[Line] = []

        def tick() -> datetime:
            nonlocal t
            t += timedelta(milliseconds=rng.randint(20, 900))
            return t

        out.append(self._entry(rng, tick(), width))
        body = max(1, width - len(_prefix(t, "INFO", _BOOK_SVC, width)))
        for text in wrap(para.text, body):
            segs = (
                (_prefix(tick(), "INFO", _BOOK_SVC, width), "dim"),
                (text, "book"),
            )
            out.append(Line(segs, True, pos))
        out.append(self._entry(rng, tick(), width))
        return out

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        return progress_bar_label(done, total, label="Tailing")

