from __future__ import annotations

import argparse
import platform
import sys
from pathlib import Path

from fcc import __version__
from fcc.errors import NoInput
from fcc.masks import MASK_NAMES
from fcc.masks.code import LANGS
from fcc.state import MAX_NOISE, Config, last_entry, load_config, load_progress, save_config, state_dir


def _version() -> str:
    target = "x86_64-pc-windows-msvc" if sys.platform == "win32" else f"{platform.machine()}-unknown-linux-gnu"
    return f"fcc {__version__} ({target})"


def _positive_float(text: str) -> float:
    try:
        value = float(text)
    except ValueError:
        raise argparse.ArgumentTypeError(f"invalid number: {text!r}") from None
    if not value > 0 or value == float("inf"):
        raise argparse.ArgumentTypeError(f"must be a positive number: {text!r}")
    return value


def _parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="fcc", description="fcc build tool")
    p.add_argument("--version", action="version", version=_version())
    sub = p.add_subparsers(dest="cmd", metavar="command")
    b = sub.add_parser("build", help="build a target")
    b.add_argument("file", nargs="?", type=Path, help="input file (default: last used)")
    b.add_argument("--mask", choices=MASK_NAMES, help="output style")
    b.add_argument("--lang", choices=LANGS, help="language for the code style")
    b.add_argument("--noise", type=int, choices=range(MAX_NOISE + 1), help="amount of filler output")
    b.add_argument("--auto", action="store_true", default=None, help="advance automatically")
    b.add_argument("--speed", type=_positive_float, help="auto advance speed")
    return p


def run_app(path: Path | None, config: Config, state_dir: Path) -> None:
    from fcc.ui.app import FccApp

    FccApp(path, config, state_dir).run()


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    sdir = state_dir()
    config = load_config(sdir)
    for name in ("mask", "lang", "noise", "auto", "speed"):
        value = getattr(args, name, None)
        if value is not None:
            setattr(config, name, value)
    save_config(sdir, config)

    path: Path | None = getattr(args, "file", None)
    if path is None:
        last = last_entry(load_progress(sdir))
        if last is None:
            from fcc.ui.app import resolve_mask

            mask, _ = resolve_mask(config)
            for line in mask.error(NoInput("no input files"), 80):
                print(line.plain, file=sys.stderr)
            return 1
        path = Path(last[1].path)
    run_app(path, config, sdir)
    return 0
