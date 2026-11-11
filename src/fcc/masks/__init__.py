from __future__ import annotations

from fcc.masks.base import Line, Mask
from fcc.masks.code import LANGS, CodeMask

MASK_NAMES: tuple[str, ...] = ("code", "log", "diff", "test", "build")

MASKS: dict[str, type] = {"code": CodeMask}


def registered_mask_names() -> tuple[str, ...]:
    return tuple(MASKS)


def get_mask(name: str, lang: str = "python") -> Mask:
    if name not in MASKS:
        raise ValueError(f"unknown or unregistered mask: {name!r}")
    return MASKS[name](lang)


__all__ = ["Line", "Mask", "CodeMask", "LANGS", "MASK_NAMES", "MASKS", "get_mask", "registered_mask_names"]
