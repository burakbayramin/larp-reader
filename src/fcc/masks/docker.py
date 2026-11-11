from __future__ import annotations

from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, Segment, chrome, error_lines, progress_bar_label
from fcc.masks.palette import BLUE, CONTROL, CYAN, DIM, FG, GREEN, YELLOW
from fcc.text import wrap

_BOOK_SVC = "worker-1"
_SERVICES = {
    "api-1": YELLOW,
    "db-1": BLUE,
    "web-1": GREEN,
    "worker-1": CYAN,
    "redis-1": CONTROL,
}
_SVC_WIDTH = max(len(s) for s in _SERVICES)
_WIDE = 30  # below this the service name is dropped, only the bar stays
_MESSAGES = {
    "api-1": (
        "INFO:     172.18.0.5:41234 - \"GET /health HTTP/1.1\" 200 OK",
        "INFO:     172.18.0.5:41250 - \"POST /v1/jobs HTTP/1.1\" 202 Accepted",
        "INFO:     Application startup complete.",
        "WARNING:  slow response from upstream (812 ms)",
    ),
    "db-1": (
        "LOG:  checkpoint starting: time",
        "LOG:  checkpoint complete: wrote 42 buffers (0.3%)",
        "LOG:  database system is ready to accept connections",
        "LOG:  autovacuum: processing table \"public.jobs\"",
    ),
    "web-1": (
        "172.18.0.1 - - [11/Nov/2026:09:14:02 +0000] \"GET / HTTP/1.1\" 200 612",
        "172.18.0.1 - - [11/Nov/2026:09:14:03 +0000] \"GET /static/app.js HTTP/1.1\" 304 0",
        "2026/11/11 09:14:05 [notice] 1#1: signal process started",
    ),
    "worker-1": (
        "[tasks] job 4821 received",
        "[tasks] job 4821 succeeded in 0.84s",
        "[beat] scheduler: sending due task cleanup",
    ),
    "redis-1": (
        "1:M 11 Nov 2026 09:14:00.112 * Background saving started by pid 42",
        "1:M 11 Nov 2026 09:14:00.215 * DB saved on disk",
        "1:M 11 Nov 2026 09:14:00.311 * Ready to accept connections tcp",
    ),
}


def _prefix(svc: str, width: int) -> tuple[Segment, ...]:
    if width >= _WIDE:
        return ((f"{svc:<{_SVC_WIDTH}} | ", _SERVICES[svc]),)
    return (("| ", _SERVICES[svc]),)


def _prefix_len(width: int) -> int:
    return sum(len(t) for t, _ in _prefix(_BOOK_SVC, width))


class DockerMask:
    """`docker compose up`: every paragraph is logged by one container."""

    name = "docker"

    def __init__(self, lang: str = "python") -> None:
        self.lang = lang

    def header(self, rng: Random, width: int) -> list[Line]:
        names = ", ".join(sorted(_SERVICES))
        return [
            chrome(width, ("[+] Running 5/5", FG)),
            chrome(width, (f"Attaching to {names}", DIM)),
        ]

    def noise(self, rng: Random, width: int) -> Line:
        svc = rng.choice(tuple(_SERVICES))
        return chrome(width, *_prefix(svc, width), (rng.choice(_MESSAGES[svc]), FG))

    def line_count(self, para: Paragraph, width: int) -> int:
        return len(wrap(para.text, max(1, width - _prefix_len(width))))

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]:
        prefix = _prefix(_BOOK_SVC, width)
        body = max(1, width - _prefix_len(width))
        return [Line((*prefix, (text, f"book {FG}")), True, pos) for text in wrap(para.text, body)]

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        return progress_bar_label(done, total, label="Extracting")
