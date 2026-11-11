"""Config and reading-progress persistence."""
from __future__ import annotations

import json
import math
import os
import tempfile
from dataclasses import asdict, dataclass
from pathlib import Path

from fcc.masks import LANGS, MASK_NAMES

MIN_SPEED = 0.2
MAX_SPEED = 20.0
MAX_NOISE = 5


def state_dir() -> Path:
    appdata = os.environ.get("APPDATA")
    base = Path(appdata) / "fcc" if appdata else Path.home() / ".config" / "fcc"
    base.mkdir(parents=True, exist_ok=True)
    return base


@dataclass
class Config:
    mask: str = "code"
    lang: str = "python"
    noise: int = 2
    speed: float = 1.5
    auto: bool = False


@dataclass
class Entry:
    path: str
    chapter: int
    paragraph: int
    bookmarks: list[list[int]]
    updated_at: str


def _atomic_write(path: Path, obj) -> None:
    fd, tmp = tempfile.mkstemp(dir=path.parent, prefix=path.name + ".", suffix=".tmp")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(obj, f, ensure_ascii=False, indent=2)
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def _read_json(path: Path):
    """Return parsed JSON, or None if missing/corrupt (corrupt -> .bak)."""
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return None
    except UnicodeDecodeError:
        text = None
    try:
        if text is None:
            raise ValueError("invalid UTF-8")
        obj = json.loads(text)
        if not isinstance(obj, dict):
            raise ValueError("expected a JSON object")
        return obj
    except ValueError:
        os.replace(path, path.with_name(path.name + ".bak"))
        return None


def _number(value) -> float | None:
    """A finite number from an int/float or a numeric string; None otherwise."""
    if isinstance(value, bool):
        return None
    if isinstance(value, str):
        try:
            value = float(value)
        except ValueError:
            return None
    if not isinstance(value, (int, float)) or not math.isfinite(value):
        return None
    return value


def _valid_config(obj: dict) -> Config:
    """Config from untrusted JSON: each bad field falls back to (or is clamped into) its valid range."""
    cfg = Config()
    if obj.get("mask") in MASK_NAMES:
        cfg.mask = obj["mask"]
    if obj.get("lang") in LANGS:
        cfg.lang = obj["lang"]
    noise = _number(obj.get("noise"))
    if noise is not None:
        cfg.noise = min(MAX_NOISE, max(0, int(noise)))
    speed = _number(obj.get("speed"))
    if speed is not None and speed > 0:
        cfg.speed = min(MAX_SPEED, max(MIN_SPEED, float(speed)))
    if isinstance(obj.get("auto"), bool):
        cfg.auto = obj["auto"]
    return cfg


def load_config(dir: Path) -> Config:
    obj = _read_json(Path(dir) / "config.json")
    if obj is None:
        return Config()
    return _valid_config(obj)


def save_config(dir: Path, cfg: Config) -> None:
    _atomic_write(Path(dir) / "config.json", asdict(cfg))


def _index(value) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _entry(obj) -> Entry | None:
    """One progress entry from untrusted JSON; None if a required field is missing or bad.

    Unknown keys are ignored, missing optional keys get defaults and malformed
    bookmarks are dropped, so one odd entry never costs the other books' progress.
    """
    if not isinstance(obj, dict):
        return None
    path, chapter, paragraph = obj.get("path"), obj.get("chapter"), obj.get("paragraph")
    if not isinstance(path, str) or not _index(chapter) or not _index(paragraph):
        return None
    marks = obj.get("bookmarks")
    bookmarks = [
        list(m)
        for m in (marks if isinstance(marks, list) else [])
        if isinstance(m, list) and len(m) == 2 and all(_index(x) for x in m)
    ]
    updated_at = obj.get("updated_at")
    return Entry(path, chapter, paragraph, bookmarks, updated_at if isinstance(updated_at, str) else "")


def load_progress(dir: Path) -> dict[str, Entry]:
    obj = _read_json(Path(dir) / "progress.json")
    if obj is None:
        return {}
    entries = {k: _entry(v) for k, v in obj.items()}
    return {k: e for k, e in entries.items() if e is not None}


def save_progress(dir: Path, data: dict[str, Entry]) -> None:
    _atomic_write(Path(dir) / "progress.json",
                  {k: asdict(v) for k, v in data.items()})


def last_entry(data: dict[str, Entry]) -> tuple[str, Entry] | None:
    if not data:
        return None
    return max(data.items(), key=lambda kv: kv[1].updated_at)
