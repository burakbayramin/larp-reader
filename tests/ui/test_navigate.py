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


# -- chapters ----------------------------------------------------------------


async def test_brackets_jump_between_chapters(tmp_path):
    app = make_app(tmp_path, make_md(tmp_path))
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("right_square_bracket")
        assert app.session.pos == (1, 0)
        await pilot.press("right_square_bracket", "right_square_bracket")
        assert app.session.pos == (2, 0)  # no chapter after the last
        await pilot.press("left_square_bracket")
        assert app.session.pos == (1, 0)
        await pilot.press("j", "j", "j")
        assert (app.session.pos, app.session.line_offset) != ((1, 0), 0)
        await pilot.press("left_square_bracket")
        assert app.session.pos == (1, 0) and app.session.line_offset == 0
        await pilot.press("left_square_bracket")
        assert app.session.pos == (0, 0)


async def test_menu_lists_sections_and_jumps_to_one(tmp_path):
    app = make_app(tmp_path, make_md(tmp_path))
    async with app.run_test(size=(80, 40)) as pilot:
        await pilot.press("colon")
        assert isinstance(app.screen, MenuScreen)
        sections = app.screen.query_one("#sections", OptionList)
        assert sections.option_count == 3
        assert "Kısım 2" in str(sections.get_option_at_index(2).prompt)
        sections.focus()
        await pilot.press("down", "down", "enter")
        assert not isinstance(app.screen, MenuScreen)
        assert app.session.pos == (2, 0)
