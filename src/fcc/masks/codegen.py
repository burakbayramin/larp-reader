"""Large pools of generated filler, read in a shuffled order so nothing repeats soon.

Drawing filler with `rng.choice` from a short list repeats lines within a
screen or two.  A `Deck` instead holds thousands of distinct items built from
templates and word lists, shuffled once with a fixed seed: reading it by a
running index gives every item once before any comes back, and the same index
always gives the same item, so a redrawn screen looks exactly as before.
"""
from __future__ import annotations

from itertools import product
from random import Random


class Deck:
    """Distinct items in a fixed shuffled order; `deck[i]` cycles through all of them."""

    def __init__(self, items, seed: str) -> None:
        unique = list(dict.fromkeys(items))
        Random(seed).shuffle(unique)
        self._items = unique

    def __len__(self) -> int:
        return len(self._items)

    def __getitem__(self, i: int) -> str:
        return self._items[i % len(self._items)]


# -- vocabulary -------------------------------------------------------------------

_VARS = (
    "chunk", "offset", "buf", "state", "count", "entry", "result", "payload", "node",
    "token", "span", "limit", "cursor", "batch", "record", "header", "digest", "delta",
    "window", "target", "source", "pending", "retries", "elapsed", "marker", "frame",
)
_ATTRS = (
    "_store", "_cache", "_index", "_queue", "_lock", "_hits", "_misses", "_pending",
    "_handlers", "_config", "_stats", "_buffer", "_registry", "_spans", "_clock",
    "_backoff", "_seen", "_sink", "_pool", "_limits",
)
_FUNCS = (
    "load", "flush", "evict", "resolve", "encode", "decode", "parse_header", "compact",
    "merge", "validate", "normalize", "dispatch", "rebuild", "snapshot", "refresh",
    "drain", "acquire", "release", "schedule", "collect", "split_chunks", "hydrate",
    "serialize", "checksum", "prune", "warm_up", "rotate", "reconcile",
)
_KEYS = (
    "id", "name", "size", "offset", "kind", "path", "version", "status", "owner",
    "created_at", "ttl", "checksum", "encoding", "priority", "region", "tags",
)
_TYPES = ("str", "int", "bytes", "float", "bool", "Path", "Span", "Node", "Entry", "Frame")
_NUMS = ("0", "1", "2", "8", "16", "64", "128", "1024", "4096", "30.0", "0.5")

_STATEMENT_TEMPLATES = (
    "{v} = self.{a}.get({k!r})",
    "{v} = self.{f}({w})",
    "{v} = {w}[{n}:]",
    "self.{a}[{k!r}] = {v}",
    "self.{a}.pop({v}, None)",
    "if {v} is None:",
    "if not self.{a}:",
    "if len({v}) > {n}:",
    "for {v} in self.{a}:",
    "for {v}, {w} in self.{a}.items():",
    "while {v} < {n}:",
    "return self.{f}({v})",
    "return {v}",
    "return {v}.{k}",
    "{v} += {n}",
    "self.{a} += {n}",
    "{v}: {t} = {w}",
    "{v} = {t}({w})",
    "{v} = {w} or {n}",
    "{v} = [{w} for {w} in self.{a} if {w}]",
    "{v} = {{{k!r}: {w}}}",
    "logger.debug({v!r}, {w})",
    "logger.info(\"{f} done\", extra={{{k!r}: {v}}})",
    "raise ValueError(f\"bad {k}: {{{v}!r}}\")",
    "assert {v} is not None",
    "with self.{a}:",
    "await self.{f}({v})",
    "{v} = await self.{f}({w})",
    "yield from self.{f}({v})",
    "del self.{a}[{v}]",
    "self.{a}.append({v})",
    "self.{a}.extend({v})",
    "{v} = sorted({w}, key=len)",
    "{v} = max({w}, {n})",
    "{v} = min({v}, {n})",
    "{v}, {w} = divmod({v}, {n})",
    "{v} = self.{a}.setdefault({k!r}, [])",
    "{v} = {w}.encode(\"utf-8\")",
    "{v} = {w}.strip()",
    "self.{f}({v}, {k}={w})",
)
_ARGS = (
    "self",
    "self, key: str",
    "self, {v}: {t}",
    "self, {v}: {t}, {w}: int = {n}",
    "self, *, timeout: float = 30.0",
    "cls, {v}: {t}",
    "self, {v}: {t} | None = None",
)
_SIG_TEMPLATES = (
    "def {f}({args}) -> None:",
    "def {f}({args}) -> {t}:",
    "async def {f}({args}) -> {t}:",
    "def _{f}({args}) -> bool:",
)

_DIRS = ("src/core", "src/api", "src/db", "src/workers", "src/cli", "src/io", "src/auth", "src/sync")
_MODULES = (
    "cache", "index", "queue", "scheduler", "handlers", "routes", "models", "session",
    "config", "storage", "parser", "encoder", "retry", "limits", "metrics", "events",
    "registry", "snapshot", "chunker", "compactor", "loader", "pipeline", "tokens",
    "backoff", "locks", "streams", "watcher", "migrate", "health", "spans",
)

_COMMAND_TEMPLATES = (
    "pytest -q tests/test_{m}.py",
    "pytest -q tests/test_{m}.py -k {f}",
    "ruff check {d}/{m}.py",
    "mypy {d}/{m}.py",
    "python -m pytest tests/test_{m}.py::test_{f} -x",
    "rg \"def {f}\" {d}",
    "git diff --stat {d}",
    "git log --oneline -5 -- {d}/{m}.py",
)

_TASK_TEMPLATES = (
    "Add tests for {m}.{f}",
    "Refactor {f} in {m}",
    "Handle empty input in {f}",
    "Update {m} docs",
    "Fix flaky test_{f}",
    "Type-check {m}",
)


def _fill(template: str, **slots) -> str:
    return template.format(**slots)


def _statements() -> list[str]:
    out = []
    for i, (tpl, v, w) in enumerate(product(_STATEMENT_TEMPLATES, _VARS, _VARS)):
        if v == w:
            continue
        # the remaining slots walk their own lists, so each (template, v, w) gets a fresh mix
        out.append(_fill(
            tpl, v=v, w=w,
            a=_ATTRS[i % len(_ATTRS)],
            f=_FUNCS[i % len(_FUNCS)],
            k=_KEYS[i % len(_KEYS)],
            t=_TYPES[i % len(_TYPES)],
            n=_NUMS[i % len(_NUMS)],
        ))
    return out


def _signatures() -> list[str]:
    out = []
    for i, (tpl, f, args) in enumerate(product(_SIG_TEMPLATES, _FUNCS, _ARGS)):
        v, w = _VARS[i % len(_VARS)], _VARS[(i + 7) % len(_VARS)]
        filled = args.format(v=v, w=w, t=_TYPES[i % len(_TYPES)], n=_NUMS[i % len(_NUMS)])
        out.append(_fill(tpl, f=f, args=filled, t=_TYPES[(i + 3) % len(_TYPES)]))
    return out


STATEMENTS = Deck(_statements(), seed="statements")
SIGNATURES = Deck(_signatures(), seed="signatures")
FILES = Deck([f"{d}/{m}.py" for d, m in product(_DIRS, _MODULES)], seed="files")
COMMANDS = Deck(
    [
        _fill(t, m=m, d=_DIRS[i % len(_DIRS)], f=_FUNCS[i % len(_FUNCS)])
        for i, (t, m) in enumerate(product(_COMMAND_TEMPLATES, _MODULES))
    ],
    seed="commands",
)
TASKS = Deck([_fill(t, m=m, f=f) for t, m, f in product(_TASK_TEMPLATES, _MODULES, _FUNCS[:6])], seed="tasks")
