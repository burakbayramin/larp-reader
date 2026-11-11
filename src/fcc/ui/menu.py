from __future__ import annotations

from dataclasses import dataclass

from textual import events
from textual.app import ComposeResult
from textual.binding import Binding
from textual.screen import ModalScreen
from textual.widgets import Input, OptionList, Static

from fcc.book import Pos
from fcc.masks import MASK_NAMES
from fcc.masks.code import LANGS
from fcc.state import HIDE_STYLES

WPMS = (120, 160, 200, 240, 280, 320, 400, 500, 650, 800)
NOISE_LEVELS = (0, 1, 2, 3, 4, 5)


@dataclass(frozen=True)
class Panic:
    pass


@dataclass(frozen=True)
class Apply:
    mask: str
    lang: str
    wpm: int
    noise: int
    hide: str = "stream"


@dataclass(frozen=True)
class OpenFile:
    path: str


@dataclass(frozen=True)
class Jump:
    pos: Pos


MenuResult = Panic | Apply | OpenFile | Jump


class _Choice(Static, can_focus=True):
    """One `name: value` line; left/right cycle through the values."""

    DEFAULT_CSS = """
    _Choice { height: 1; }
    _Choice:focus { text-style: reverse; }
    """
    BINDINGS = [
        Binding("left", "cycle(-1)", show=False),
        Binding("right", "cycle(1)", show=False),
    ]

    def __init__(self, name: str, options: tuple, current, **kwargs) -> None:
        opts = list(options)
        if current not in opts:
            opts = sorted(opts + [current])
        super().__init__("", **kwargs)
        self._label = name
        self.options = opts
        self.index = opts.index(current)
        self._show()

    @property
    def value(self):
        return self.options[self.index]

    def _show(self) -> None:
        self.update(f"{self._label}: < {self.value} >")

    def action_cycle(self, delta: int) -> None:
        self.index = (self.index + delta) % len(self.options)
        self._show()


class _PathInput(Input):
    """Text input that still lets the panic key through (printable keys skip bindings)."""

    def on_key(self, event: events.Key) -> None:
        if event.key == "grave_accent":
            event.prevent_default()
            event.stop()
            self.screen.dismiss(Panic())


def section_label(n: int, title: str | None, width: int = 48) -> str:
    label = f"{n:>3} {' '.join((title or '-').split())}"
    return label if len(label) <= width else label[: width - 1] + "~"


class MenuScreen(ModalScreen[MenuResult]):
    """Plain-text settings screen styled to blend in with the terminal."""

    DEFAULT_CSS = """
    MenuScreen { background: $background; align: left top; }
    MenuScreen Static { height: 1; }
    MenuScreen Input { border: none; height: 1; padding: 0; background: $background; }
    MenuScreen OptionList {
        border: none; height: auto; max-height: 10; padding: 0; background: $background;
    }
    MenuScreen OptionList:focus { border: none; }
    """
    BINDINGS = [
        Binding("escape", "close", show=False, priority=True),
        Binding("grave_accent", "panic", show=False, priority=True),
        Binding("enter", "apply", show=False),
        Binding("down", "app.focus_next", show=False),
        Binding("up", "app.focus_previous", show=False),
    ]

    def __init__(
        self,
        mask: str,
        lang: str,
        wpm: int,
        noise: int,
        marks: list[tuple[str, Pos]],
        sections: list[tuple[str, Pos]] | None = None,
        hide: str = "stream",
    ) -> None:
        super().__init__()
        self._mask, self._lang, self._wpm, self._noise = mask, lang, wpm, noise
        self._marks = marks
        self._sections = sections or []
        self._hide = hide

    def compose(self) -> ComposeResult:
        yield Static("fcc config")
        yield _PathInput(placeholder="input: path", id="path")
        yield _Choice("target", MASK_NAMES, self._mask, id="target")
        yield _Choice("lang", LANGS, self._lang, id="lang")
        yield _Choice("rate", WPMS, self._wpm, id="rate")
        yield _Choice("verbosity", NOISE_LEVELS, self._noise, id="noise")
        yield _Choice("on hide", HIDE_STYLES, self._hide, id="hide")
        yield Static("marks:" if self._marks else "marks: none")
        yield OptionList(*[label for label, _ in self._marks], id="marks")
        yield Static("sections:" if len(self._sections) > 1 else "sections: 1")
        yield OptionList(*[label for label, _ in self._sections], id="sections")

    def on_mount(self) -> None:
        marks = self.query_one("#marks", OptionList)
        if self._marks:
            marks.highlighted = 0
        else:
            marks.display = False
        sections = self.query_one("#sections", OptionList)
        if len(self._sections) > 1:
            sections.highlighted = 0
        else:
            sections.display = False
        self.query_one("#path", Input).focus()

    def action_panic(self) -> None:
        self.dismiss(Panic())

    def action_close(self) -> None:
        self.dismiss(None)

    def action_apply(self) -> None:
        self.dismiss(
            Apply(
                self.query_one("#target", _Choice).value,
                self.query_one("#lang", _Choice).value,
                self.query_one("#rate", _Choice).value,
                self.query_one("#noise", _Choice).value,
                self.query_one("#hide", _Choice).value,
            )
        )

    def on_input_submitted(self, event: Input.Submitted) -> None:
        event.stop()
        path = event.value.strip().strip("\"'")
        if path:
            self.dismiss(OpenFile(path))

    def on_option_list_option_selected(self, event: OptionList.OptionSelected) -> None:
        event.stop()
        source = self._sections if event.option_list.id == "sections" else self._marks
        self.dismiss(Jump(source[event.option_index][1]))
