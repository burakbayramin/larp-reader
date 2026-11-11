import json
import re

import pytest
from textual.widgets import Input, OptionList, Static

from fcc.masks import MASK_NAMES
from fcc.state import Config, load_config, load_progress
from fcc.ui.app import FccApp
from fcc.ui.menu import MenuScreen
from tests.ui.test_app_auto_panic import book_words, shown
from tests.ui.test_app_manual import make_txt, status_text

FORBIDDEN = re.compile(r"book|reader|okuyucu", re.IGNORECASE)


def make_app(tmp_path, path=None):
    (tmp_path / "st").mkdir(exist_ok=True)
    config = Config(mask="code", lang="python", noise=0)
    return FccApp(path or make_txt(tmp_path), config, tmp_path / "st")


def in_menu(app) -> bool:
    return isinstance(app.screen, MenuScreen)


async def test_bookmark_added_once(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        await pilot.press("down", "down")
        pos = app.session.pos
        await pilot.press("b")
        assert status_text(app) == "cache saved"
        assert app._flash_timer is not None
        await pilot.press("b")
        entries = load_progress(tmp_path / "st")
        assert [e.bookmarks for e in entries.values()] == [[list(pos)]]
        app._clear_flash()  # what the 2 s timer fires
        assert status_text(app) != "cache saved"
        assert "Indexing" in status_text(app) or "/" in status_text(app)
        await pilot.press("down", "b")
        marks = next(iter(load_progress(tmp_path / "st").values())).bookmarks
        assert marks == [list(pos), list(app.session.pos)]


async def test_bookmark_survives_save_failure(tmp_path, monkeypatch):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        def boom(*a, **k):
            raise OSError("disk full")

        monkeypatch.setattr("fcc.ui.app.save_progress", boom)
        await pilot.press("b")
        assert app.is_running
        assert [0, 0] in app._bookmarks


async def test_menu_changes_mask_and_persists(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        await pilot.press("colon")
        await pilot.pause()
        assert in_menu(app)
        for ident in ("target", "lang", "rate", "noise"):
            app.screen.query_one(f"#{ident}").focus()
            await pilot.press("right")
        await pilot.press("enter")
        await pilot.pause()
        assert not in_menu(app)
        assert app._mask.name == MASK_NAMES[1] == "log"
        assert app.session.mask is app._mask
        assert app.session.noise == 1
        assert app.speed > 1.5 or app.speed != Config().speed
        saved = load_config(tmp_path / "st")
        assert (saved.mask, saved.noise) == ("log", 1)
        assert saved.speed == app.speed
        assert saved.lang == app.config.lang != "python"


async def test_menu_jump_to_bookmark(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        await pilot.press("down", "down", "down")
        marked = app.session.pos
        await pilot.press("b")
        await pilot.press("up", "up", "up", "up")
        assert app.session.pos != marked
        await pilot.press("colon")
        await pilot.pause()
        marks = app.screen.query_one("#marks", OptionList)
        labels = [str(marks.get_option_at_index(i).prompt) for i in range(marks.option_count)]
        assert labels == [f"L{app.session.book.index_of(marked) + 1}"]
        marks.focus()
        await pilot.press("enter")
        await pilot.pause()
        assert not in_menu(app)
        assert app.session.pos == marked
        assert any(ln.pos == marked for ln in app.feed.visible_lines())


async def test_menu_open_new_file(tmp_path):
    other = tmp_path / "other"
    other.mkdir()
    new = make_txt(other, n=4)
    new.write_text("Zebra quokka xylophone.\n\nSecond walrus paragraph.", encoding="utf-8")
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        old_id = app.session.book.id
        await pilot.press("colon")
        await pilot.pause()
        field = app.screen.query_one("#path", Input)
        field.focus()
        field.value = f'"{new}"'
        await pilot.press("enter")
        await pilot.pause()
        assert not in_menu(app)
        assert app.session.book.id != old_id
        assert "zebra" in shown(app)


async def test_menu_open_bad_file_keeps_session(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        session = app.session
        await pilot.press("colon")
        await pilot.pause()
        field = app.screen.query_one("#path", Input)
        field.focus()
        field.value = str(tmp_path / "missing.epub")
        await pilot.press("enter")
        await pilot.pause()
        assert not in_menu(app)
        assert app.session is session
        assert "error:" in shown(app)
        assert any(ln.is_book for ln in app.feed.visible_lines())


@pytest.mark.parametrize("key", ["escape", "grave_accent"])
async def test_panic_from_menu_closes_menu(tmp_path, key):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        words = book_words(app.session.book)
        await pilot.press("colon")
        await pilot.pause()
        assert in_menu(app)
        await pilot.press(key)
        await pilot.pause()
        assert not in_menu(app)
        assert app.panic
        assert not any(w in shown(app) for w in words)
        assert not any(ln.is_book for ln in app.feed.visible_lines())


async def test_menu_text_has_no_forbidden_words(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        await pilot.press("down", "b", "colon")
        await pilot.pause()
        texts = []
        for w in app.screen.query("*"):
            if isinstance(w, Static):
                texts.append(str(w.render()))
            if isinstance(w, Input):
                texts += [w.placeholder, w.value]
            if isinstance(w, OptionList):
                texts += [str(w.get_option_at_index(i).prompt) for i in range(w.option_count)]
        joined = "\n".join(texts)
        assert "fcc config" in joined and "L2" in joined
        assert not FORBIDDEN.search(joined), joined
