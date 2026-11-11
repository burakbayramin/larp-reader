import re

from fcc.state import Config, load_progress
from fcc.ui import app as app_mod
from fcc.ui.app import FccApp
from tests.ui.test_app_manual import make_txt


def cfg(noise: int = 3, **kw) -> Config:
    return Config(mask="code", lang="python", noise=noise, **kw)


def book_words(book) -> set[str]:
    words: set[str] = set()
    for ch in book.chapters:
        for p in ch.paragraphs:
            words |= {w.lower() for w in re.findall(r"[A-Za-z]{4,}", p.text)}
    return words


def shown(app) -> str:
    return " ".join(ln.plain for ln in app.feed.history_lines()).lower()


def book_count(app) -> int:
    return sum(ln.is_book for ln in app.feed.history_lines())


async def test_panic_removes_every_book_word(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        words = book_words(app.session.book)
        assert words
        before = shown(app)
        assert any(w in before for w in words)  # book text WAS visible
        await pilot.press("a")
        for _ in range(5):
            app.tick()
        await pilot.press("escape")
        await pilot.pause()
        assert app.panic
        for _ in range(30):
            app.tick()
        await pilot.pause()
        text = shown(app)
        assert text.strip()
        assert not any(w in text for w in words)
        vis = " ".join(ln.plain for ln in app.feed.visible_lines()).lower()
        assert not any(w in vis for w in words)
        assert not any(ln.is_book for ln in app.feed.history_lines())
        rendered = app.feed.render_text().plain.lower()
        assert not any(w in rendered for w in words)


async def test_panic_toggle_returns_to_same_pos(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("down", "down")
        pos = app.session.pos
        await pilot.press("grave_accent")
        assert app.panic
        for _ in range(5):
            app.tick()
        assert app.session.pos == pos
        await pilot.press("grave_accent")
        await pilot.pause()
        assert not app.panic and not app.auto
        assert app.session.pos == pos
        assert any(ln.pos == pos for ln in app.feed.visible_lines())
        # auto mode is restored as auto
        await pilot.press("a")
        await pilot.press("escape", "escape")
        assert app.auto and not app.panic
        assert app.session.pos == pos


async def test_auto_tick_appends_one_book_line_plus_noise(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(noise=3), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("a")
        assert app.auto
        for _ in range(6):
            before_book = book_count(app)
            before_all = len(app.feed.history_lines())
            app.tick()
            assert book_count(app) == before_book + 1
            assert len(app.feed.history_lines()) > before_all


async def test_auto_is_bottom_anchored(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("a")
        for _ in range(40):
            app.tick()
        assert app.feed.visible_lines() == app.feed.history_lines()[-len(app.feed.visible_lines()):]
        assert app.feed.visible_lines()[-1] == app.feed.history_lines()[-1]


async def test_buffer_capped_at_2000(tmp_path):
    app = FccApp(make_txt(tmp_path, n=400), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("a")
        for _ in range(1500):
            app.tick()
        assert len(app.feed.history_lines()) == 2000


async def test_speed_clamped(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        for _ in range(30):
            await pilot.press("+")
        assert app.speed == 20.0
        for _ in range(60):
            await pilot.press("-")
        assert app.speed == 0.2


async def test_speed_step_is_1_25(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(speed=2.0), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("+")
        assert app.speed == 2.5
        await pilot.press("-")
        assert app.speed == 2.0


async def test_config_auto_and_speed_apply_at_startup(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(auto=True, speed=4.0), tmp_path / "st")
    async with app.run_test(size=(80, 24)):
        assert app.auto
        assert app.speed == 4.0
        assert app._timer is not None


async def test_auto_reaches_end_and_stops(tmp_path):
    app = FccApp(make_txt(tmp_path, n=3), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("a")
        for _ in range(200):
            app.tick()
        assert app.session.progress() == (3, 3)
        n = len(app.feed.history_lines())
        app.tick()
        app.tick()
        assert len(app.feed.history_lines()) == n
        assert app._timer is None


async def test_manual_keys_do_nothing_in_panic(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("escape")
        await pilot.press("space", "down")
        assert app.session.progress()[0] == 1
        assert not any(ln.is_book for ln in app.feed.history_lines())


async def test_autosave_writes_progress(tmp_path):
    st = tmp_path / "st"
    st.mkdir()
    app = FccApp(make_txt(tmp_path), cfg(), st)
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("down", "down")
        app._autosave()
        saved = load_progress(st)
        assert (saved[app.session.book.id].chapter, saved[app.session.book.id].paragraph) == app.session.pos


async def test_save_failure_does_not_crash_and_reports_once(tmp_path, monkeypatch):
    def boom(*a, **k):
        raise OSError("disk full")

    monkeypatch.setattr(app_mod, "save_progress", boom)
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("down")
        app._autosave()
        app._autosave()
        errs = [ln for ln in app.feed.history_lines() if ln.plain.startswith("error:")]
        assert len(errs) == 1
        assert any("disk full" in ln.plain for ln in app.feed.history_lines())
        await pilot.press("q")  # still exits without raising
    assert not app.is_running
