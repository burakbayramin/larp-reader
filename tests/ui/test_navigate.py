"""Page back, chapter jumps, search, bookmark jumps and the fake shell."""
from pathlib import Path

from textual.widgets import Input, OptionList

from fcc.state import Config, load_config
from fcc.ui.app import FccApp
from fcc.ui.menu import MenuScreen
from tests.ui.test_app_auto_panic import book_words, shown
from tests.ui.test_app_manual import make_txt, status_text


def make_app(tmp_path: Path, path: Path | None = None, **kw) -> FccApp:
    (tmp_path / "st").mkdir(exist_ok=True)
    config = Config(mask="code", lang="python", noise=0, **kw)
    return FccApp(path or make_txt(tmp_path), config, tmp_path / "st")


def make_md(tmp_path: Path) -> Path:
    parts = []
    for c in range(3):
        parts.append(f"# Kısım {c}")
        parts += [f"chapter{c} para{i} " + "lorem ipsum " * 30 for i in range(4)]
    path = tmp_path / "parts.md"
    path.write_text("\n\n".join(parts), encoding="utf-8")
    return path


def screen(app) -> list:
    return [(ln.plain, ln.pos) for ln in app.feed.visible_lines()]


# -- page back ---------------------------------------------------------------


async def test_page_back_returns_to_the_previous_screens(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(80, 24)) as pilot:
        first = screen(app)
        await pilot.press("space")
        second = screen(app)
        await pilot.press("space")
        await pilot.press("pageup")
        assert screen(app) == second
        await pilot.press("u")
        assert screen(app) == first
        await pilot.press("u")
        assert screen(app) == first  # already at the top


async def test_pagedown_pages_like_space(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("space")
        after_space = screen(app)
        await pilot.press("u", "pagedown")
        assert screen(app) == after_space
