"""Editor colours (VS Code Dark+) and syntax highlighting for mask chrome."""

from __future__ import annotations

from functools import lru_cache

from pygments import lex
from pygments.lexer import Lexer
from pygments.lexers import get_lexer_by_name
from pygments.token import Token

from fcc.masks.base import Segment, crop

BG = "#1e1e1e"
FG = "#d4d4d4"
GUTTER = "#858585"
COMMENT = "#6a9955"
KEYWORD = "#569cd6"
CONTROL = "#c586c0"
FUNC = "#dcdcaa"
TYPE = "#4ec9b0"
STRING = "#ce9178"
NUMBER = "#b5cea8"
VAR = "#9cdcfe"
CONST = "#4fc1ff"

# terminal output
GREEN = "#3fb950"
YELLOW = "#d29922"
RED = "#f85149"
BLUE = "#58a6ff"
CYAN = "#39c5cf"
DIM = "#6e7681"

_CONTROL_WORDS = frozenset(
    "if else elif for while return match import from use package try catch except finally "
    "await switch case break continue throw raise yield using defer foreach lock".split()
)


@lru_cache(maxsize=None)
def _lexer(lang: str) -> Lexer:
    return get_lexer_by_name(lang, stripnl=False, ensurenl=False)


def _style(ttype, text: str, following: str) -> str:
    if ttype in Token.Comment:
        return COMMENT
    if ttype in Token.String:
        return STRING
    if ttype in Token.Number:
        return NUMBER
    if ttype in Token.Keyword.Type:
        return TYPE
    if ttype in Token.Keyword or ttype in Token.Operator.Word:
        return CONTROL if text in _CONTROL_WORDS else KEYWORD
    if ttype in Token.Name.Function or ttype in Token.Name.Decorator:
        return FUNC
    if ttype in Token.Name.Class or ttype in Token.Name.Namespace:
        return TYPE
    if ttype in Token.Name.Builtin.Pseudo:
        return KEYWORD
    if ttype in Token.Name.Builtin:
        return TYPE if text[:1].isupper() else FUNC
    if ttype in Token.Name.Constant:
        return CONST
    if ttype in Token.Name:
        if following.startswith("("):
            return FUNC
        if text[:1].isupper():
            return CONST if text.isupper() and len(text) > 1 else TYPE
        return VAR
    return FG


@lru_cache(maxsize=None)
def highlight(lang: str, code: str) -> tuple[tuple[Segment, ...], ...]:
    """Syntax-coloured segments for each line of `code`."""
    tokens = list(lex(code, _lexer(lang)))
    lines: list[list[Segment]] = [[]]
    for i, (ttype, text) in enumerate(tokens):
        following = tokens[i + 1][1] if i + 1 < len(tokens) else ""
        style = _style(ttype, text, following)
        for j, part in enumerate(text.split("\n")):
            if j:
                lines.append([])
            if not part:
                continue
            if lines[-1] and lines[-1][-1][1] == style:
                part = lines[-1].pop()[0] + part
            lines[-1].append((part, style))
    return tuple(tuple(line) for line in lines)
