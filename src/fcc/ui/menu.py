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

SPEEDS = (0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 8.0, 12.0, 20.0)
NOISE_LEVELS = (0, 1, 2, 3, 4, 5)


@dataclass(frozen=True)
class Panic:
    pass


@dataclass(frozen=True)
class Apply:
    mask: str
    lang: str
    speed: float
    noise: int


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
        Binding("escape,grave_accent", "panic", show=False, priority=True),
        Binding("enter", "apply", show=False),
        Binding("down", "app.focus_next", show=False),
        Binding("up", "app.focus_previous", show=False),
    ]

    def __init__(
        self,
        mask: str,
        lang: str,
        speed: float,
        noise: int,
        marks: list[tuple[str, Pos]],
    ) -> None:
        super().__init__()
        self._mask, self._lang, self._speed, self._noise = mask, lang, speed, noise
        self._marks = marks

    def compose(self) -> ComposeResult:
        yield Static("fcc config")
        yield _PathInput(placeholder="input: path", id="path")
        yield _Choice("target", MASK_NAMES, self._mask, id="target")
        yield _Choice("lang", LANGS, self._lang, id="lang")
        yield _Choice("rate", SPEEDS, self._speed, id="rate")
        yield _Choice("verbosity", NOISE_LEVELS, self._noise, id="noise")
        yield Static("marks:" if self._marks else "marks: none")
        yield OptionList(*[label for label, _ in self._marks], id="marks")

    def on_mount(self) -> None:
        marks = self.query_one("#marks", OptionList)
        if self._marks:
            marks.highlighted = 0
        else:
            marks.display = False
        self.query_one("#path", Input).focus()

    def action_panic(self) -> None:
        self.dismiss(Panic())

    def action_apply(self) -> None:
        self.dismiss(
            Apply(
                self.query_one("#target", _Choice).value,
                self.query_one("#lang", _Choice).value,
                self.query_one("#rate", _Choice).value,
                self.query_one("#noise", _Choice).value,
            )
        )

    def on_input_submitted(self, event: Input.Submitted) -> None:
        event.stop()
        path = event.value.strip().strip("\"'")
        if path:
            self.dismiss(OpenFile(path))

    def on_option_list_option_selected(self, event: OptionList.OptionSelected) -> None:
        event.stop()
        self.dismiss(Jump(self._marks[event.option_index][1]))
