from __future__ import annotations

from rich.text import Text
from textual.widget import Widget

from fcc.masks.base import Line

_INTERNAL_MARKER = "book"
MAX_LINES = 2000


def clean_style(style: str) -> str:
    """Drop the internal book-text marker so it never reaches Rich."""
    return " ".join(tok for tok in style.split() if tok != _INTERNAL_MARKER)


class Feed(Widget):
    """Scrolling text area showing masked lines, top-anchored at `top`."""

    DEFAULT_CSS = """
    Feed {
        height: 1fr;
        width: 100%;
    }
    """

    def __init__(self, **kwargs) -> None:
        super().__init__(**kwargs)
        self._lines: list[Line] = []
        self._top = 0
        self._bottom = False
        self._height_override: int | None = None

    def set_bottom_anchored(self, bottom: bool) -> None:
        """Bottom-anchored feeds show the last rows, like a scrolling terminal."""
        self._bottom = bottom
        self.refresh()

    def set_view_height(self, height: int) -> None:
        """Rows available for display; used until the widget has a real size."""
        self._height_override = max(1, height)

    def _height(self) -> int:
        if self.size.height > 0:
            return self.size.height
        return self._height_override or 1

    def set_lines(self, lines: list[Line]) -> None:
        self._lines = list(lines)
        self._top = 0
        self.refresh()

    def append(self, lines: list[Line]) -> None:
        self._lines += lines
        if len(self._lines) > MAX_LINES:
            del self._lines[: len(self._lines) - MAX_LINES]
        self.refresh()

    def clear(self) -> None:
        self._lines = []
        self._top = 0
        self.refresh()

    def visible_lines(self) -> list[Line]:
        if self._bottom:
            return self._lines[-self._height() :]
        return self._lines[self._top : self._top + self._height()]

    def history_lines(self) -> list[Line]:
        return list(self._lines)

    def render_text(self) -> Text:
        text = Text(no_wrap=True, overflow="crop")
        for i, line in enumerate(self.visible_lines()):
            if i:
                text.append("\n")
            for seg, style in line.segments:
                text.append(seg, clean_style(style) or None)
        return text

    def render(self) -> Text:
        return self.render_text()
