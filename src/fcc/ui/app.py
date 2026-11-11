from __future__ import annotations

from datetime import datetime
from pathlib import Path
from random import Random

from textual import events
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.widgets import Static

from fcc.book import Book, Pos
from fcc.errors import FccError, NoInput
from fcc.loaders import load_book
from fcc.masks import Mask, get_mask
from fcc.masks.base import Line, plain_line
from fcc.session import Session
from fcc.state import Config, Entry, last_entry, load_progress, save_progress
from fcc.ui.feed import Feed

MIN_WIDTH = 40
_WARNING = "warning: window too narrow"


def resolve_mask(config: Config) -> tuple[Mask, FccError | None]:
    """The configured mask, or the code mask plus the problem if it can't be built."""
    try:
        return get_mask(config.mask, config.lang), None
    except ValueError as e:
        return get_mask("code"), FccError(str(e))


class FccApp(App):
    TITLE = "fcc"
    SUB_TITLE = ""
    CSS = """
    Screen { layout: vertical; }
    #status { height: 1; dock: bottom; }
    """
    BINDINGS = [
        Binding("space", "page", show=False),
        Binding("down,j", "step(1)", show=False),
        Binding("up,k", "step(-1)", show=False),
        Binding("q", "quit_and_save", show=False),
    ]

    def __init__(self, path: Path | None, config: Config, state_dir: Path) -> None:
        super().__init__()
        self._path = Path(path) if path is not None else None
        self.config = config
        self.state_dir = Path(state_dir)
        self.session: Session | None = None
        self.feed = Feed()
        self._mask, self._mask_error = resolve_mask(config)
        self._notice: list[Line] = []
        self._entry_path: str | None = None
        self._cols = 80
        self._rows = 24

    def compose(self) -> ComposeResult:
        yield self.feed
        yield Static("", id="status")

    # -- lifecycle --------------------------------------------------------

    def on_mount(self) -> None:
        self._cols, self._rows = self.size.width, self.size.height
        if self._mask_error is not None:
            self._notice += self._mask.error(self._mask_error, self._width())
        try:
            self._open()
        except (FccError, OSError) as e:
            err = e if isinstance(e, FccError) else FccError(str(e))
            self._notice += self._mask.error(err, self._width())
        self._redraw()

    def on_resize(self, event: events.Resize) -> None:
        # App.size lags behind the event, so track the event's size ourselves.
        self._cols, self._rows = event.size.width, event.size.height
        if self.session is not None:
            self.session.width = self._width()
        self._redraw()

    def _open(self) -> None:
        progress = load_progress(self.state_dir)
        path = self._path
        if path is None:
            last = last_entry(progress)
            if last is None:
                raise NoInput("no input files")
            path = Path(last[1].path)
        book = load_book(path)
        self._entry_path = str(path.resolve())
        pos: Pos = (0, 0)
        entry = progress.get(book.id)
        if entry is not None:
            pos = _valid_pos(book, (entry.chapter, entry.paragraph))
        self.session = Session(book, self._mask, self.config.noise, self._width(), pos)

    # -- drawing ----------------------------------------------------------

    def _width(self) -> int:
        return max(1, self._cols)

    def _view_height(self) -> int:
        return max(1, self._rows - 1)

    def _redraw(self) -> None:
        self.feed.set_view_height(self._view_height())
        lines: list[Line] = []
        if self._width() < MIN_WIDTH:
            lines.append(plain_line(_WARNING[: self._width()], "yellow"))
        lines += self._notice
        s = self.session
        if s is not None:
            if s.book.index_of(s.pos) == 0:
                lines += s.mask.header(Random(s.book.id))
            lines += s.lines_from(s.pos, self._view_height())
        self.feed.set_lines(lines)
        status = self.query_one("#status", Static)
        if s is None:
            status.update("")
        else:
            done, total = s.progress()
            status.update(s.mask.progress_label(done, total, self._width()))

    # -- actions ----------------------------------------------------------

    def _move(self, n: int) -> None:
        if self.session is None:
            return
        self._notice = []
        if n > 0:
            self.session.advance(n)
        else:
            self.session.back(-n)
        self._redraw()

    def action_page(self) -> None:
        shown = {ln.pos for ln in self.feed.visible_lines() if ln.is_book}
        self._move(max(1, len(shown)))

    def action_step(self, n: int) -> None:
        self._move(n)

    def action_quit_and_save(self) -> None:
        self._save()
        self.exit()

    def _save(self) -> None:
        s = self.session
        if s is None or self._entry_path is None:
            return
        data = load_progress(self.state_dir)
        data[s.book.id] = Entry(
            path=self._entry_path,
            chapter=s.pos[0],
            paragraph=s.pos[1],
            bookmarks=data[s.book.id].bookmarks if s.book.id in data else [],
            updated_at=datetime.now().isoformat(timespec="seconds"),
        )
        save_progress(self.state_dir, data)


def _valid_pos(book: Book, pos: Pos) -> Pos:
    try:
        book.paragraph(pos)
        return pos
    except IndexError:
        return (0, 0)
