from __future__ import annotations

from fcc.masks.base import Line, Mask
from fcc.masks.build import BuildMask
from fcc.masks.code import LANGS, CodeMask
from fcc.masks.diff import DiffMask
from fcc.masks.log import LogMask
from fcc.masks.test import TestMask

MASK_NAMES: tuple[str, ...] = ("code", "log", "diff", "test", "build")

MASKS: dict[str, type] = {
    "code": CodeMask,
    "log": LogMask,
    "diff": DiffMask,
    "test": TestMask,
    "build": BuildMask,
}


def registered_mask_names() -> tuple[str, ...]:
    return tuple(MASKS)


def get_mask(name: str, lang: str = "python") -> Mask:
    if name not in MASKS:
        raise ValueError(f"unknown or unregistered mask: {name!r}")
    return MASKS[name](lang)


__all__ = ["Line", "Mask", "CodeMask", "LogMask", "DiffMask", "TestMask", "BuildMask", "LANGS", "MASK_NAMES", "MASKS", "get_mask", "registered_mask_names"]
