from __future__ import annotations

from datetime import datetime, timedelta
from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, error_lines, plain_line, progress_bar_label
from fcc.text import wrap

_EPOCH = datetime(2026, 11, 11, 9, 0, 0)
_BOOK_SVC = "svc.worker"
_SVC_WIDTH = 10
_PREFIX_WIDTH = 24 + 1 + 5 + 1 + _SVC_WIDTH + 2  # ts, level, service, gutters
_SERVICES = ("svc.worker", "svc.api", "svc.cache", "svc.queue", "svc.auth")
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


def _prefix(t: datetime, level: str, svc: str) -> str:
    return f"{_stamp(t)} {level:<5} {svc:<{_SVC_WIDTH}}  "


class LogMask:
    name = "log"

    def __init__(self, lang: str = "python") -> None:
        self.lang = lang

    def header(self, rng: Random) -> list[Line]:
        return [plain_line("-- Logs begin at Wed 2026-11-11 09:00:00 UTC --", "dim")]

    def _entry(self, rng: Random, t: datetime, width: int) -> Line:
        level = rng.choices(["INFO", "DEBUG", "WARN"], weights=[6, 2, 1])[0]
        text = _prefix(t, level, rng.choice(_SERVICES)) + rng.choice(_MESSAGES[level])
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
        body = max(1, width - _PREFIX_WIDTH)
        for text in wrap(para.text, body):
            segs = (
                (_prefix(tick(), "INFO", _BOOK_SVC), "dim"),
                (text, "book"),
            )
            out.append(Line(segs, True, pos))
        out.append(self._entry(rng, tick(), width))
        return out

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        return progress_bar_label(done, total, label="Tailing")

