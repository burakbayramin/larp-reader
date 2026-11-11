from __future__ import annotations

from datetime import datetime
from pathlib import Path
from random import Random

from textual import events
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.timer import Timer
from textual.widgets import Static

from fcc.book import Book, Pos
from fcc.errors import FccError, NoInput
from fcc.loaders import load_book
from fcc.masks import MASK_NAMES, Mask, get_mask
from fcc.masks.base import Line, plain_line
from fcc.session import Session
from fcc.state import Config, Entry, last_entry, load_progress, save_config, save_progress
from fcc.ui.feed import Feed
from fcc.ui.menu import Apply, Jump, MenuResult, MenuScreen, OpenFile, Panic

MIN_WIDTH = 40
MIN_SPEED = 0.2
MAX_SPEED = 20.0
MAX_NOISE = 5
SPEED_STEP = 1.25
PANIC_SPEEDUP = 3
AUTOSAVE_SECONDS = 10
FLASH_SECONDS = 2
_SAVED_MSG = "cache saved"
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
        Binding("a", "toggle_auto", show=False),
        Binding("plus", "faster", show=False),
        Binding("minus", "slower", show=False),
        Binding("m", "cycle_mask", show=False),
        Binding("n", "noise(1)", show=False),
        Binding("N", "noise(-1)", show=False),
        Binding("b", "bookmark", show=False),
        Binding("colon", "open_menu", show=False),
        Binding("escape,grave_accent", "toggle_panic", show=False),
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
        self.auto = False
        self.panic = False
        self.speed = min(MAX_SPEED, max(MIN_SPEED, config.speed))
        self._timer: Timer | None = None
        self._queue: list[Line] = []
        self._panic_rng = Random()
        self._save_error_shown = False
        self._bookmarks: list[list[int]] = []
        self._flash: str | None = None
        self._flash_timer: Timer | None = None

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
        self.set_interval(AUTOSAVE_SECONDS, self._autosave)
        if self.config.auto and self.session is not None:
            self.auto = True
        self._redraw()
        self._restart_timer()

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
        self._load(path, progress)

    def _load(self, path: Path, progress: dict[str, Entry]) -> None:
        book = load_book(path)  # raises before any state changes
        pos: Pos = (0, 0)
        entry = progress.get(book.id)
        if entry is not None:
            pos = _valid_pos(book, (entry.chapter, entry.paragraph))
        self._entry_path = str(path.resolve())
        self._bookmarks = [list(m) for m in entry.bookmarks] if entry is not None else []
        self.session = Session(book, self._mask, self.config.noise, self._width(), pos)

    # -- drawing ----------------------------------------------------------

    def _width(self) -> int:
        return max(1, self._cols)

    def _view_height(self) -> int:
        return max(1, self._rows - 1)

    def _redraw(self) -> None:
        self.feed.set_view_height(self._view_height())
        if self.panic:
            return
        self.feed.set_bottom_anchored(self.auto)
        self._queue = []
        lines: list[Line] = []
        if self._width() < MIN_WIDTH:
            lines.append(plain_line(_WARNING[: self._width()], "yellow"))
        lines += self._notice
        s = self.session
        if s is not None:
            if s.book.index_of(s.pos) == 0:
                lines += s.mask.header(Random(s.book.id))
            if self.auto:
                lines += s.paragraph_lines(s.pos)
            else:
                lines += s.lines_from(s.pos, self._view_height())
        self.feed.set_lines(lines)
        self._update_status()

    def _update_status(self) -> None:
        status = self.query_one("#status", Static)
        s = self.session
        if self._flash is not None:
            status.update(self._flash)
        elif s is None:
            status.update("")
        else:
            done, total = s.progress()
            status.update(s.mask.progress_label(done, total, self._width()))

    def _clear_flash(self) -> None:
        self._flash = None
        if self._flash_timer is not None:
            self._flash_timer.stop()
            self._flash_timer = None
        self._update_status()

    # -- actions ----------------------------------------------------------

    def _move(self, n: int) -> None:
        if self.session is None or self.auto or self.panic:
            return
        self._notice = []
        if n > 0:
            self.session.advance(n)
        else:
            self.session.back(-n)
        self._redraw()

    def action_page(self) -> None:
        if self.auto or self.panic:
            return
        shown = {ln.pos for ln in self.feed.visible_lines() if ln.is_book}
        self._move(max(1, len(shown)))

    def action_step(self, n: int) -> None:
        self._move(n)

    def action_quit_and_save(self) -> None:
        self._try_save()
        self.exit()

    def action_toggle_auto(self) -> None:
        if self.session is None or self.panic:
            return
        self.auto = not self.auto
        self._notice = []
        self._redraw()
        self._restart_timer()

    def action_faster(self) -> None:
        self._set_speed(self.speed * SPEED_STEP)

    def action_slower(self) -> None:
        self._set_speed(self.speed / SPEED_STEP)

    def _set_speed(self, speed: float) -> None:
        self.speed = min(MAX_SPEED, max(MIN_SPEED, speed))
        self._restart_timer()

    def action_toggle_panic(self) -> None:
        self.panic = not self.panic
        self._stop_timer()
        self.feed.clear()
        self._queue = []
        if self.panic:
            self.feed.set_bottom_anchored(True)
            self.feed.append(
                [self._mask.noise(self._panic_rng, self._width()) for _ in range(self._view_height())]
            )
        else:
            self._redraw()
        self._restart_timer()

    def action_bookmark(self) -> None:
        s = self.session
        if s is None or self.panic:
            return
        mark = list(s.pos)
        if mark not in self._bookmarks:
            self._bookmarks.append(mark)
            self._try_save()
        self._flash = _SAVED_MSG
        if self._flash_timer is not None:
            self._flash_timer.stop()
        self._flash_timer = self.set_timer(FLASH_SECONDS, self._clear_flash)
        self._update_status()

    def action_open_menu(self) -> None:
        if self.panic or isinstance(self.screen, MenuScreen):
            return
        s = self.session
        marks = (
            [(f"L{s.book.index_of(tuple(m)) + 1}", (m[0], m[1])) for m in self._bookmarks]
            if s is not None
            else []
        )
        self.push_screen(
            MenuScreen(self._mask.name, self.config.lang, self.speed, self.config.noise, marks),
            self._on_menu_result,
        )

    def _on_menu_result(self, result: MenuResult | None) -> None:
        if isinstance(result, Panic):
            if not self.panic:
                self.action_toggle_panic()
        elif isinstance(result, Apply):
            self._apply_settings(result)
        elif isinstance(result, OpenFile):
            self._open_from_menu(Path(result.path).expanduser())
        elif isinstance(result, Jump) and self.session is not None:
            self.session.pos = _valid_pos(self.session.book, result.pos)
            self._notice = []
            self._redraw()

    def _apply_settings(self, r: Apply) -> None:
        self._mask = get_mask(r.mask, r.lang)
        self.config.mask, self.config.lang = r.mask, r.lang
        self.config.noise, self.config.speed = r.noise, r.speed
        if self.session is not None:
            self.session.mask = self._mask
            self.session.noise = r.noise
        self._persist_config()
        self._set_speed(r.speed)
        self._redraw()

    def _open_from_menu(self, path: Path) -> None:
        self._try_save()
        try:
            self._load(path, load_progress(self.state_dir))
        except (FccError, OSError) as e:
            err = e if isinstance(e, FccError) else FccError(str(e))
            self._notice = self._mask.error(err, self._width())
        else:
            self._notice = []
            self._queue = []
        self._redraw()
        self._restart_timer()

    def action_cycle_mask(self) -> None:
        i = MASK_NAMES.index(self._mask.name) if self._mask.name in MASK_NAMES else -1
        name = MASK_NAMES[(i + 1) % len(MASK_NAMES)]
        self._mask = get_mask(name, self.config.lang)
        if self.session is not None:
            self.session.mask = self._mask
        self.config.mask = name
        self._persist_config()
        self._redraw()

    def action_noise(self, delta: int) -> None:
        level = min(MAX_NOISE, max(0, self.config.noise + delta))
        self.config.noise = level
        if self.session is not None:
            self.session.noise = level
        self._persist_config()
        self._redraw()

    def _persist_config(self) -> None:
        try:
            save_config(self.state_dir, self.config)
        except OSError:
            pass  # best effort: the choice still applies for this run

    # -- timer / streaming ------------------------------------------------

    def _stop_timer(self) -> None:
        if self._timer is not None:
            self._timer.stop()
            self._timer = None

    def _restart_timer(self) -> None:
        self._stop_timer()
        if self.panic:
            interval = 1 / (self.speed * PANIC_SPEEDUP)
        elif self.auto and self.session is not None:
            interval = 1 / self.speed
        else:
            return
        self._timer = self.set_interval(interval, self.tick)

    def tick(self) -> None:
        """One timer step: a panic noise line, or the next book line plus its noise."""
        if self.panic:
            self.feed.append([self._mask.noise(self._panic_rng, self._width())])
            return
        s = self.session
        if not self.auto or s is None:
            return
        out: list[Line] = []
        finished = False
        while True:
            if not self._queue:
                if not s.advance():
                    finished = True
                    break
                self._queue = s.paragraph_lines(s.pos)
            ln = self._queue.pop(0)
            out.append(ln)
            if ln.is_book:
                break
        if out:
            self.feed.append(out)
        if finished:
            self._stop_timer()

    def _save(self) -> None:
        s = self.session
        if s is None or self._entry_path is None:
            return
        data = load_progress(self.state_dir)
        data[s.book.id] = Entry(
            path=self._entry_path,
            chapter=s.pos[0],
            paragraph=s.pos[1],
            bookmarks=[list(m) for m in self._bookmarks],
            updated_at=datetime.now().isoformat(timespec="seconds"),
        )
        save_progress(self.state_dir, data)

    def _try_save(self) -> None:
        try:
            self._save()
        except OSError as e:
            if self._save_error_shown:
                return
            self._save_error_shown = True
            lines = self._mask.error(FccError(f"could not save state: {e}"), self._width())
            self._notice += lines
            if self.auto and not self.panic:
                self.feed.append(lines)
            else:
                self._redraw()

    def _autosave(self) -> None:
        self._try_save()


def _valid_pos(book: Book, pos: Pos) -> Pos:
    try:
        book.paragraph(pos)
        return pos
    except IndexError:
        return (0, 0)
