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


# -- search ------------------------------------------------------------------


async def type_text(pilot, text: str) -> None:
    for ch in text:
        await pilot.press("space" if ch == " " else ch)


async def test_slash_searches_and_highlights(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("slash")
        bar = app.query_one("#searchbar")
        assert bar.display and app.query_one("#search", Input).has_focus
        await type_text(pilot, "number 7")
        await pilot.press("enter")
        assert not bar.display
        assert app.session.pos == (0, 7)
        hits = [
            text
            for ln in app.feed.visible_lines()
            for text, style in ln.segments
            if "reverse" in style.split()
        ]
        assert "number 7" in hits


async def test_f_and_shift_f_walk_matches_and_wrap(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("slash")
        await type_text(pilot, "LOREM")
        await pilot.press("enter")
        assert app.session.pos == (0, 0)
        await pilot.press("f")
        assert app.session.pos == (0, 1)
        await pilot.press("F", "F")
        assert app.session.pos == (0, 9)  # wrapped to the end
        assert "continuing" in status_text(app)


async def test_missing_pattern_reports_and_stays(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("slash")
        await type_text(pilot, "zzz")
        await pilot.press("enter")
        assert app.session.pos == (0, 0)
        assert "Pattern not found" in status_text(app)


async def test_escape_cancels_search_without_opening_the_panel(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("slash", "x", "escape")
        assert not app.query_one("#searchbar").display
        assert not app.query_one("#help").display
        await pilot.press("j")
        assert app.session.line_offset == 1 or app.session.pos != (0, 0)


async def test_backtick_in_search_hides_everything(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("slash", "grave_accent")
        assert app.panic
        assert not app.query_one("#searchbar").display


# -- bookmark jumps ----------------------------------------------------------


async def test_quote_keys_jump_between_marks(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(80, 24)) as pilot:
        app._bookmarks = [[0, 6], [0, 2]]
        await pilot.press("apostrophe")
        assert app.session.pos == (0, 2)
        await pilot.press("apostrophe")
        assert app.session.pos == (0, 6)
        await pilot.press("apostrophe")
        assert app.session.pos == (0, 2)  # wraps
        await pilot.press("quotation_mark")
        assert app.session.pos == (0, 6)


async def test_quote_without_marks_says_so(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("apostrophe")
        assert app.session.pos == (0, 0)
        assert "cache" in status_text(app)


# -- fake shell --------------------------------------------------------------


async def test_shell_hide_shows_a_prompt_that_takes_commands(tmp_path):
    app = make_app(tmp_path, hide="shell")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("grave_accent")
        assert app.panic
        prompt = app.query_one("#shellbar")
        assert prompt.display and app.query_one("#shell", Input).has_focus
        before = len(app.feed.history_lines())
        await pilot.pause(0.5)
        assert len(app.feed.history_lines()) == before  # a shell waits, it does not stream
        await type_text(pilot, "ls")
        await pilot.press("enter")
        lines = [ln.plain for ln in app.feed.history_lines()]
        assert any(l.endswith(" ls") for l in lines)
        assert "src" in " ".join(lines)
        words = book_words(app.session.book)
        assert not any(w in shown(app).split() for w in words - {"number"})
        await type_text(pilot, "jk")
        assert app.session.pos == (0, 0) and app.session.line_offset == 0
        await pilot.press("grave_accent")
        assert not app.panic and not prompt.display


async def test_shell_clear_and_unknown_commands(tmp_path):
    app = make_app(tmp_path, hide="shell")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("grave_accent")
        await type_text(pilot, "clear")
        await pilot.press("enter")
        assert app.feed.history_lines() == []
        await type_text(pilot, "frobnicate")
        await pilot.press("enter")
        assert "frobnicate" in shown(app) and ("not found" in shown(app) or "not recognized" in shown(app))


async def test_hide_style_is_a_menu_setting(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(80, 40)) as pilot:
        await pilot.press("colon")
        app.screen.query_one("#hide").focus()
        await pilot.press("right", "enter")
        assert app.config.hide == "shell"
        assert load_config(tmp_path / "st").hide == "shell"


def test_hide_config_is_validated(tmp_path):
    import json

    (tmp_path / "config.json").write_text(json.dumps({"hide": "shell"}))
    assert load_config(tmp_path).hide == "shell"
    (tmp_path / "config.json").write_text(json.dumps({"hide": "nope"}))
    assert load_config(tmp_path).hide == "stream"
