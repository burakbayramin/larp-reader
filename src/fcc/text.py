import re


def normalize_paragraphs(raw: list[str]) -> list[str]:
    out = []
    for p in raw:
        p = re.sub(r"(?<=\w)-[ \t]*\r?\n[ \t]*", "", p)
        p = " ".join(p.split())
        if p:
            out.append(p)
    return out


def wrap(text: str, width: int) -> list[str]:
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
