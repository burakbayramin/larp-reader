"""Config and reading-progress persistence."""
from __future__ import annotations

import json
import os
import tempfile
from dataclasses import asdict, dataclass, fields
from pathlib import Path


def state_dir() -> Path:
    appdata = os.environ.get("APPDATA")
    base = Path(appdata) / "fcc" if appdata else Path.home() / ".config" / "fcc"
    base.mkdir(parents=True, exist_ok=True)
    return base


@dataclass
class Config:
    mask: str = "build"
    lang: str = "rust"
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
    try:
        obj = json.loads(text)
        if not isinstance(obj, dict):
            raise ValueError("expected a JSON object")
        return obj
    except ValueError:
        os.replace(path, path.with_name(path.name + ".bak"))
        return None


def load_config(dir: Path) -> Config:
    obj = _read_json(Path(dir) / "config.json")
    if obj is None:
        return Config()
    known = {f.name for f in fields(Config)}
    return Config(**{k: v for k, v in obj.items() if k in known})


def save_config(dir: Path, cfg: Config) -> None:
    _atomic_write(Path(dir) / "config.json", asdict(cfg))


def load_progress(dir: Path) -> dict[str, Entry]:
    obj = _read_json(Path(dir) / "progress.json")
    if obj is None:
        return {}
    try:
        return {k: Entry(**v) for k, v in obj.items()}
    except TypeError:
        os.replace(Path(dir) / "progress.json",
                   Path(dir) / "progress.json.bak")
        return {}


def save_progress(dir: Path, data: dict[str, Entry]) -> None:
    _atomic_write(Path(dir) / "progress.json",
                  {k: asdict(v) for k, v in data.items()})


def last_entry(data: dict[str, Entry]) -> tuple[str, Entry] | None:
    if not data:
        return None
    return max(data.items(), key=lambda kv: kv[1].updated_at)
