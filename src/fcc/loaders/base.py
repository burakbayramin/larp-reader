from __future__ import annotations

from pathlib import Path
from typing import Protocol

from fcc.book import Book


class Loader(Protocol):
    name: str
    extensions: tuple[str, ...]

    def can_load(self, path: Path, head: bytes) -> bool: ...

    def load(self, path: Path) -> Book: ...
