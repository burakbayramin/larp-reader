from __future__ import annotations

import getpass
import os
import socket
from datetime import datetime
from pathlib import Path
from random import Random

from rich.text import Text
from textual import events
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal
from textual.timer import Timer
from textual.widgets import Input, Static

from fcc.book import Book, Pos, match_spans
from fcc.errors import FccError, InputUnavailable, NoInput
from fcc.loaders import load_book, supported_extensions
from fcc.masks import LANGS, MASK_NAMES, Mask, get_mask
from fcc.masks.base import Line, plain_line
from fcc.pace import line_delay, step_wpm, words_in
from fcc.session import Session
from fcc.state import (
    MAX_NOISE,
    Config,
    Entry,
    last_entry,
    load_progress,
    save_config,
    save_progress,
)
from fcc.ui.feed import Feed
from fcc.ui.menu import Apply, Jump, MenuResult, MenuScreen, OpenFile, Panic, section_label

MIN_WIDTH = 40
PANIC_RATE = 4.5  # filler lines per second while hidden
AUTOSAVE_SECONDS = 10
FLASH_SECONDS = 2
_SAVED_MSG = "cache saved"
_RATE_MSG = "throttle {}/min"
_NO_MARKS_MSG = "cache empty"
_NOT_FOUND_MSG = "Pattern not found"
_WRAP_MSG = {False: "search hit BOTTOM, continuing at TOP", True: "search hit TOP, continuing at BOTTOM"}
HIGHLIGHT = "reverse"
_WARNING = "warning: window too narrow"


def resolve_mask(config: Config) -> tuple[Mask, FccError | None]:
    """The configured mask, or the code mask plus the problem if it can't be built."""
    try:
        return get_mask(config.mask, config.lang), None
    except ValueError as e:
        return get_mask("code"), FccError(str(e))


LISTED_INPUTS = 9  # one per digit key

_KEYS = (
    ("space", "page down"),
    ("u, pgup", "page up"),
    ("j / k", "line down / up"),
    ("] / [", "next / prev section"),
    ("/", "find"),
    ("f / F", "find next / prev"),
    ("a", "auto scroll"),
    ("+ / -", "reading rate"),
    ("m / M", "next / prev view"),
    ("l / L", "next / prev language"),
    ("n / N", "more / less filler"),
    ("o / O", "next / prev input"),
    ("1-9", "open listed input"),
    ("b", "mark this place"),
    ("' / \"", "next / prev mark"),
    (":", "config"),
    ("`", "hide / unhide"),
    ("esc", "this panel"),
    ("q", "quit"),
)


class FccApp(App):
    TITLE = "fcc"
    SUB_TITLE = ""
    ENABLE_COMMAND_PALETTE = False  # a recognisable Textual popup; Esc would close it, not panic
    AUTO_FOCUS = None  # keys go to the app's bindings until a bar is opened
    CSS = """
    Screen { layout: vertical; background: #1e1e1e; color: #d4d4d4; }
    Feed { background: #1e1e1e; color: #d4d4d4; }
    #status { height: 1; dock: bottom; background: #1e1e1e; color: #858585; }
    Screen { layers: base overlay; }
    #help {
        layer: overlay; dock: right; display: none;
        width: 38; height: 100%; padding: 1 2;
        background: #252526; color: #cccccc; border-left: solid #3c3c3c;
    }
    .bar { dock: bottom; height: 1; display: none; background: #1e1e1e; }
    .bar Static { width: auto; height: 1; color: #d4d4d4; }
    .bar Input, .bar Input:focus {
        width: 1fr; height: 1; border: none; padding: 0;
        background: #1e1e1e; color: #d4d4d4;
    }
    """
    BINDINGS = [
        Binding("space,pagedown", "page", show=False),
        Binding("u,pageup", "page_back", show=False),
        Binding("right_square_bracket", "chapter(1)", show=False),
        Binding("left_square_bracket", "chapter(-1)", show=False),
        Binding("slash", "search", show=False),
        Binding("f", "find_next(1)", show=False),
        Binding("F", "find_next(-1)", show=False),
        Binding("apostrophe", "jump_mark(1)", show=False),
        Binding("quotation_mark", "jump_mark(-1)", show=False),
        Binding("down,j", "step(1)", show=False),
        Binding("up,k", "step(-1)", show=False),
        Binding("q", "quit_and_save", show=False),
        Binding("ctrl+q", "quit_and_save", show=False, priority=True),  # Textual's quit skips the save
        Binding("a", "toggle_auto", show=False),
        Binding("plus", "faster", show=False),
        Binding("minus", "slower", show=False),
        Binding("m", "cycle_mask(1)", show=False),
        Binding("M", "cycle_mask(-1)", show=False),
        Binding("l", "cycle_lang(1)", show=False),
        Binding("L", "cycle_lang(-1)", show=False),
        Binding("n", "noise(1)", show=False),
        Binding("N", "noise(-1)", show=False),
        Binding("b", "bookmark", show=False),
        Binding("colon", "open_menu", show=False),
        Binding("o", "cycle_input(1)", show=False),
        Binding("O", "cycle_input(-1)", show=False),
        *[Binding(str(n), f"open_listed({n})", show=False) for n in range(1, 10)],
        Binding("escape", "toggle_help", show=False),
        Binding("grave_accent", "toggle_panic", show=False),
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
        self._timer: Timer | None = None
        self._queue: list[Line] = []
        self._panic_rng = Random()
        self._save_error_shown = False
        self._bookmarks: list[list[int]] = []
        self._flash: str | None = None
        self._flash_timer: Timer | None = None
        # manual mode: text lines on screen now, and whether more text follows them
        self._page_rows = 0
        self._more_below = False
        self._query = ""  # last search; its matches are highlighted
        self._last_words = 0  # words of the newest text on screen, still to be read

    def compose(self) -> ComposeResult:
        yield self.feed
        yield Static("", id="status")
        with Horizontal(id="searchbar", classes="bar"):
            yield Static("/")
            yield _BarInput(id="search")
        with Horizontal(id="shellbar", classes="bar"):
            yield Static(shell_prompt(), id="prompt")
            yield _BarInput(id="shell")
        yield Static("", id="help")

    # -- lifecycle --------------------------------------------------------

    def on_mount(self) -> None:
        self._cols, self._rows = self.size.width, self.size.height
        if self._mask_error is not None:
            self._notice += self._mask.error(self._mask_error, self._width())
        try:
            self._open()
        except (FccError, OSError) as e:
            err = e if isinstance(e, FccError) else InputUnavailable.from_os_error(e)
            self._notice += self._mask.error(err, self._width())
        self.set_interval(AUTOSAVE_SECONDS, self._autosave)
        if self.config.auto and self.session is not None:
            self.auto = True
        self._redraw()
        self._restart_timer()

    def on_resize(self, event: events.Resize) -> None:
        # App.size lags behind the event, so track the event's size ourselves.
        self._cols, self._rows = event.size.width, event.size.height
        if self.session is not None and self.session.width != self._width():
            self.session.width = self._width()
            self.session.pos = self.session.pos  # re-wrapped: restart at the paragraph's top
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
            at_top = s.book.index_of(s.pos) == 0 and (self.auto or s.line_offset == 0)
            if at_top:
                lines += s.mask.header(Random(s.book.id), self._width())
            if self.auto:
                para = s.paragraph_lines(s.pos)
                self._last_words = words_in(para)
                lines += para
            else:
                rows = max(0, self._view_height() - len(lines))
                text = s.window(rows + 1)
                self._more_below = len(text) > rows
                self._page_rows = min(rows, len(text))
                lines += text[:rows]
        self.feed.set_lines(self._highlight(lines))
        self._update_status()

    def _highlight(self, lines: list[Line]) -> list[Line]:
        """Mark the last search's matches inside the text, like grep --color."""
        if not self._query:
            return lines
        return [highlight(ln, self._query) if ln.is_book else ln for ln in lines]

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

    def _scroll(self, n: int) -> None:
        """Move the manual view by `n` rendered lines; never past the last screen."""
        if self.session is None or self.auto or self.panic:
            return
        if n > 0 and not self._more_below:
            return  # the end of the text is already on screen
        self._notice = []
        if n:
            self.session.scroll(n)
        self._redraw()

    def action_page(self) -> None:
        # the next screen starts at the first line this one did not show
        self._scroll(self._page_rows)

    def action_page_back(self) -> None:
        """The screen that ends just above this one (or the first screen)."""
        if self.session is None or self.auto or self.panic:
            return
        rows = self._view_height() - (1 if self._width() < MIN_WIDTH else 0)
        self._notice = []
        self.session.scroll(-max(1, rows))
        self._redraw()

    def _go(self, pos: Pos) -> None:
        """Show the text from the top of paragraph `pos`."""
        assert self.session is not None
        self.session.pos = pos
        self._notice = []
        self._redraw()

    def action_chapter(self, step: int) -> None:
        s = self.session
        if s is None or self.panic:
            return
        if step > 0:
            target = s.book.next_chapter(s.pos)
        elif s.pos[1] == 0 and s.line_offset > 0:
            target = s.pos  # back to the top of a section that starts here
        else:
            target = s.book.prev_chapter(s.pos)
        if target is not None:
            self._go(target)

    def action_jump_mark(self, step: int) -> None:
        s = self.session
        if s is None or self.panic:
            return
        marks: list[tuple[int, Pos]] = []
        for m in self._bookmarks:
            pos = (m[0], m[1])
            if _valid_pos(s.book, pos) == pos:
                marks.append((s.book.index_of(pos), pos))
        if not marks:
            self._flash_msg(_NO_MARKS_MSG)
            return
        marks.sort()
        here = s.book.index_of(s.pos)
        if step > 0:
            target = next((p for i, p in marks if i > here), marks[0][1])
        else:
            target = next((p for i, p in reversed(marks) if i < here), marks[-1][1])
        self._go(target)

    # -- search -----------------------------------------------------------

    def _bar(self, name: str, show: bool) -> None:
        bar = self.query_one(f"#{name}bar")
        field = self.query_one(f"#{name}", Input)
        bar.display = show
        self.query_one("#status", Static).display = not show
        if show:
            field.value = ""
            field.focus()
        elif field.has_focus:
            self.set_focus(None)

    def action_search(self) -> None:
        if self.session is None or self.panic:
            return
        self._show_help(False)
        self._bar("search", True)

    def _find(self, start: Pos, backward: bool, wrapped: bool = False) -> None:
        s = self.session
        assert s is not None
        found = s.book.find(self._query, start, backward)
        if found is None:
            self._redraw()
            self._flash_msg(_NOT_FOUND_MSG)
            return
        pos, wrapped_now = found
        self._go(pos)
        if wrapped or wrapped_now:
            self._flash_msg(_WRAP_MSG[backward])

    def action_find_next(self, step: int) -> None:
        s = self.session
        if s is None or self.panic or not self._query:
            return
        backward = step < 0
        start = s.book.prev_pos(s.pos) if backward else s.book.next_pos(s.pos)
        wrapped = start is None
        if start is None:
            order = [(ci, 0) for ci, _ in s.book.chapter_list()]
            if not order:
                return
            last_ci = order[-1][0]
            start = (last_ci, len(s.book.chapters[last_ci].paragraphs) - 1) if backward else order[0]
        self._find(start, backward, wrapped)

    def on_input_submitted(self, event: Input.Submitted) -> None:
        if event.input.id == "search":
            event.stop()
            self._bar("search", False)
            self._query = event.value.strip()
            if self._query and self.session is not None:
                self._find(self.session.pos, backward=False)
            else:
                self._redraw()
        elif event.input.id == "shell":
            event.stop()
            event.input.value = ""
            self._run_shell(event.value)

    def bar_escape(self, field: Input) -> None:
        if field.id == "search":
            self._bar("search", False)

    def action_step(self, n: int) -> None:
        self._scroll(n)

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
        self._set_wpm(step_wpm(self.config.wpm, 1))

    def action_slower(self) -> None:
        self._set_wpm(step_wpm(self.config.wpm, -1))

    def _set_wpm(self, wpm: int) -> None:
        self.config.wpm = wpm
        self._persist_config()
        if not self.panic:
            self._flash_msg(_RATE_MSG.format(wpm))
        self._restart_timer()

    # -- side panel and the folder's other files ---------------------------

    def _inputs(self) -> list[Path]:
        """The openable files next to the current one, in name order."""
        if self._entry_path is None:
            return []
        current = Path(self._entry_path)
        known = supported_extensions()
        try:
            found = [f for f in current.parent.iterdir() if f.is_file() and f.suffix.lower() in known]
        except OSError:
            found = []
        if current not in found:
            found.append(current)
        return sorted(found, key=lambda f: f.name.lower())

    def _help_text(self) -> Text:
        text = Text(no_wrap=True, overflow="ellipsis")
        text.append("keys\n", "bold #d4d4d4")
        for keys, what in _KEYS:
            text.append(f"{keys:<9}", "#dcdcaa")
            text.append(f"{what}\n", "#cccccc")
        files = self._inputs()
        if files:
            text.append("\ninputs\n", "bold #d4d4d4")
            current = Path(self._entry_path) if self._entry_path else None
            for n, f in enumerate(files[:LISTED_INPUTS], 1):
                here = f == current
                text.append(f"{'>' if here else n} ", "#dcdcaa")
                text.append(f"{f.name}\n", "bold #d4d4d4" if here else "#cccccc")
            if len(files) > LISTED_INPUTS:
                text.append(f"  +{len(files) - LISTED_INPUTS} more (o / O)\n", "#858585")
        return text

    def _show_help(self, show: bool) -> None:
        panel = self.query_one("#help", Static)
        if show:
            panel.update(self._help_text())
        panel.display = show

    def action_toggle_help(self) -> None:
        if self.panic:
            return  # hidden means hidden: only the hide key brings things back
        self._show_help(not self.query_one("#help", Static).display)

    def _switch_to(self, path: Path) -> None:
        self._open_from_menu(path)
        if self.query_one("#help", Static).display:
            self._show_help(True)

    def action_cycle_input(self, step: int = 1) -> None:
        files = self._inputs()
        if self.panic or len(files) < 2:
            return
        i = files.index(Path(self._entry_path))
        self._switch_to(files[(i + step) % len(files)])

    def action_open_listed(self, n: int) -> None:
        files = self._inputs()[:LISTED_INPUTS]
        if self.panic or not self.query_one("#help", Static).display or n > len(files):
            return
        if files[n - 1] != Path(self._entry_path):
            self._switch_to(files[n - 1])

    def action_toggle_panic(self) -> None:
        self.panic = not self.panic
        self._show_help(False)
        self._bar("search", False)
        self._stop_timer()
        self.feed.clear()
        self._queue = []
        shell = self.config.hide == "shell"
        if self.panic:
            self.feed.set_bottom_anchored(True)
            rows = self._view_height() - (1 if shell else 0)
            self.feed.append([self._mask.noise(self._panic_rng, self._width()) for _ in range(rows)])
            if shell:
                self._bar("shell", True)
        else:
            self._bar("shell", False)
            self._redraw()
        self._restart_timer()

    # -- fake shell -------------------------------------------------------

    def _run_shell(self, command: str) -> None:
        """Echo a command at the prompt and print something believable for it."""
        command = command.strip()
        out = shell_output(command, self._mask, self._panic_rng, self._width())
        if out is None:
            self.feed.clear()
            return
        prompt = self.query_one("#prompt", Static).render()
        self.feed.append([plain_line(f"{prompt}{command}", "#d4d4d4"), *out])

    def action_bookmark(self) -> None:
        s = self.session
        if s is None or self.panic:
            return
        mark = list(s.pos)
        if mark not in self._bookmarks:
            self._bookmarks.append(mark)
            self._try_save()
        self._flash_msg(_SAVED_MSG)

    def _flash_msg(self, msg: str) -> None:
        self._flash = msg
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
        sections = (
            [(section_label(n, title), (ci, 0)) for n, (ci, title) in enumerate(s.book.chapter_list(), 1)]
            if s is not None
            else []
        )
        self.push_screen(
            MenuScreen(
                self._mask.name,
                self.config.lang,
                self.config.wpm,
                self.config.noise,
                marks,
                sections,
                self.config.hide,
            ),
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
        self.config.noise, self.config.wpm = r.noise, r.wpm
        self.config.hide = r.hide
        if self.session is not None:
            self.session.mask = self._mask
            self.session.noise = r.noise
            self.session.pos = self.session.pos  # new layout: restart at the paragraph's top
        self._persist_config()
        self._restart_timer()
        self._redraw()

    def _open_from_menu(self, path: Path) -> None:
        self._try_save()
        try:
            self._load(path, load_progress(self.state_dir))
        except (FccError, OSError) as e:
            err = e if isinstance(e, FccError) else InputUnavailable.from_os_error(e)
            self._notice = self._mask.error(err, self._width())
        else:
            self._notice = []
            self._queue = []
        self._redraw()
        self._restart_timer()

    def _use_mask(self, name: str, lang: str) -> None:
        """Switch the disguise in place, staying on the current paragraph."""
        self._mask = get_mask(name, lang)
        if self.session is not None:
            self.session.mask = self._mask
            self.session.pos = self.session.pos  # new layout: restart at the paragraph's top
        self.config.mask, self.config.lang = name, lang
        self._persist_config()
        self._redraw()

    def action_cycle_mask(self, step: int = 1) -> None:
        i = MASK_NAMES.index(self._mask.name) if self._mask.name in MASK_NAMES else -1
        self._use_mask(MASK_NAMES[(i + step) % len(MASK_NAMES)], self.config.lang)

    def action_cycle_lang(self, step: int = 1) -> None:
        """Next source language; only the code mask has one, so this also returns to it."""
        i = LANGS.index(self.config.lang) if self.config.lang in LANGS else -1
        self._use_mask("code", LANGS[(i + step) % len(LANGS)])

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
        if self.panic and self.config.hide == "shell":
            return  # a shell waits for input
        if self.panic:
            self._timer = self.set_interval(1 / PANIC_RATE, self.tick)
        elif self.auto and self.session is not None:
            # the text already on screen gets its reading time before the next line
            self._timer = self.set_timer(line_delay(self._last_words, self.config.wpm), self._auto_step)

    def _auto_step(self) -> None:
        """Show the next line, then wait as long as it takes to read it."""
        self._timer = None
        self.tick()
        if self.auto and not self.panic and self._timer is None:
            self._timer = self.set_timer(line_delay(self._last_words, self.config.wpm), self._auto_step)

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
        self._last_words = words_in(out)
        if out:
            self.feed.append(self._highlight(out))
        if finished:
            # end of text: back to manual mode so the paging keys work again
            self._stop_timer()
            self.auto = False
            self._redraw()

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


class _BarInput(Input):
    """One-line input at the bottom; the hide key and Esc still work while typing."""

    def on_key(self, event: events.Key) -> None:
        if event.key == "grave_accent":
            event.prevent_default()
            event.stop()
            self.app.action_toggle_panic()
        elif event.key == "escape":
            event.prevent_default()
            event.stop()
            self.app.bar_escape(self)


def highlight(line: Line, query: str) -> Line:
    """`line` with every match of `query` inside its text segments shown reversed."""
    segments: list[tuple[str, str]] = []
    for text, style in line.segments:
        spans = match_spans(text, query) if "book" in style.split() else []
        if not spans:
            segments.append((text, style))
            continue
        at = 0
        for start, end in spans:
            if start > at:
                segments.append((text[at:start], style))
            segments.append((text[start:end], f"{style} {HIGHLIGHT}"))
            at = end
        if at < len(text):
            segments.append((text[at:], style))
    return Line(tuple(segments), line.is_book, line.pos)


def shell_prompt() -> str:
    """A prompt like the user's own shell would show for the current folder."""
    cwd = Path.cwd()
    if os.name == "nt":
        return f"PS {cwd}> "
    try:
        user = getpass.getuser()
    except Exception:  # no login name in some containers
        user = "dev"
    host = socket.gethostname().split(".")[0] or "localhost"
    if os.environ.get("SHELL", "").endswith("zsh"):
        return f"{user}@{host} {cwd.name or '/'} % "
    home = Path.home()
    shown = "~" + str(cwd)[len(str(home)):] if cwd == home or home in cwd.parents else str(cwd)
    return f"{user}@{host}:{shown}$ "


_LISTING = (("README.md", "#d4d4d4"), ("docs", "bold #58a6ff"), ("pyproject.toml", "#d4d4d4"),
            ("src", "bold #58a6ff"), ("tests", "bold #58a6ff"))
_GIT_STATUS = (
    "On branch main",
    "Your branch is up to date with 'origin/main'.",
    "",
    "nothing to commit, working tree clean",
)
_TOOLS = frozenset(
    "make cmake npm npx pnpm yarn node tsc python python3 py pip pip3 pytest uv poetry cargo "
    "rustc go dotnet mvn gradle ./gradlew docker kubectl terraform ruff mypy black eslint git".split()
)


def shell_output(command: str, mask: Mask, rng: Random, width: int) -> list[Line] | None:
    """What a shell prints for `command`; None means clear the screen."""
    words = command.split()
    if not words:
        return []
    name = words[0]
    if name in ("clear", "cls"):
        return None
    if name in ("ls", "dir"):
        segs: list[tuple[str, str]] = []
        for i, (entry, style) in enumerate(_LISTING):
            segs += [("  " if i else "", ""), (entry, style)]
        return [Line(tuple(segs), False)]
    if name == "pwd":
        return [plain_line(str(Path.cwd()))]
    if command == "git status":
        return [plain_line(t) for t in _GIT_STATUS]
    if name in _TOOLS:
        return [mask.noise(rng, width) for _ in range(rng.randint(4, 12))]
    if os.name == "nt":
        return [plain_line(f"{name}: The term '{name}' is not recognized as a name of a cmdlet, "
                           "function, script file, or executable program.", "#f85149")]
    shell = Path(os.environ.get("SHELL", "bash")).name or "bash"
    return [plain_line(f"{shell}: command not found: {name}")]
