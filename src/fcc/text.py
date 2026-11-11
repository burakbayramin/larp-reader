import re
import zlib
from random import Random


def normalize_paragraphs(raw: list[str]) -> list[str]:
    out = []
    for p in raw:
        p = re.sub(r"(?<=\w)-[ \t]*\r?\n[ \t]*", "", p)
        p = " ".join(p.split())
        if p:
            out.append(p)
    return out


def wrap(text: str, width: int) -> list[str]:
    width = max(1, width)
    lines: list[str] = []
    cur = ""
    for word in text.split():
        while len(word) > width:
            if cur:
                lines.append(cur)
                cur = ""
            lines.append(word[:width])
            word = word[width:]
        if not word:
            continue
        if not cur:
            cur = word
        elif len(cur) + 1 + len(word) <= width:
            cur += " " + word
        else:
            lines.append(cur)
            cur = word
    if cur:
        lines.append(cur)
    return lines


CODE_LINE_MAX = 88  # source lines rarely run longer, however wide the window
_BREAK_AFTER = ".,;:!?"
_SHORTEST = 0.45  # a line ends somewhere between this share of the room and all of it
_PUNCT_BREAK = 0.7  # chance to end a long-enough line at a clause boundary


def ragged(text: str, width: int) -> list[str]:
    """Words laid out like lines of source code: uneven ends, mostly at clause boundaries.

    The breaks depend only on the text and the width, so the same paragraph is
    always laid out the same way (and can be counted without being drawn).
    """
    room = max(1, min(width, CODE_LINE_MAX))
    rng = Random(zlib.crc32(f"{room}:{text}".encode()))
    shortest = max(1, int(room * _SHORTEST))
    lines: list[str] = []
    cur = ""
    target = rng.randint(shortest, room)

    def end_line() -> None:
        nonlocal cur, target
        lines.append(cur)
        cur = ""
        target = rng.randint(shortest, room)

    for word in text.split():
        while len(word) > room:
            if cur:
                end_line()
            lines.append(word[:room])
            word = word[room:]
        if not word:
            continue
        if cur and len(cur) + 1 + len(word) > target:
            end_line()
        cur = f"{cur} {word}" if cur else word
        if len(cur) >= shortest and cur[-1] in _BREAK_AFTER and rng.random() < _PUNCT_BREAK:
            end_line()
    if cur:
        lines.append(cur)
    return lines
