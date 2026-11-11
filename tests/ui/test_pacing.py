"""Auto scroll paced by words per minute, and the rate learned from manual paging."""
from fcc.pace import learn, words_in
from fcc.state import Config, load_config
from fcc.ui import app as app_mod
from fcc.ui.app import FccApp
from tests.ui.test_app_manual import make_txt


class FakeClock:
    def __init__(self) -> None:
        self.now = 1000.0

    def __call__(self) -> float:
        return self.now


def make_app(tmp_path, **kw) -> tuple[FccApp, FakeClock]:
    (tmp_path / "st").mkdir(exist_ok=True)
    app = FccApp(make_txt(tmp_path, n=30), Config(mask="log", noise=0, **kw), tmp_path / "st")
    clock = FakeClock()
    app._clock = clock
    return app, clock


async def test_auto_waits_by_the_words_on_screen_then_by_each_line(tmp_path, monkeypatch):
    calls = []
    real = app_mod.line_delay
    monkeypatch.setattr(app_mod, "line_delay", lambda w, r: calls.append((w, r)) or real(w, r))
    app, _ = make_app(tmp_path, wpm=300)
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("a")
        first_para = words_in(app.session.paragraph_lines(app.session.pos))
        assert calls[-1] == (first_para, 300)
        before = len(app.feed.history_lines())
        app._timer.stop()
        app._auto_step()
        new = app.feed.history_lines()[before:]
        assert calls[-1] == (words_in(new), 300) and words_in(new) > 0
        assert app._timer is not None  # the next line is already scheduled


async def test_paging_learns_the_reading_rate(tmp_path):
    app, clock = make_app(tmp_path, wpm=200)
    async with app.run_test(size=(80, 24)) as pilot:
        words = app._view_words
        assert words > 20
        clock.now += 20
        await pilot.press("space")
        expected = learn(200, words, 20)
        assert app.config.wpm == expected != 200
        assert load_config(tmp_path / "st").wpm == expected


async def test_quick_paging_and_line_scrolling_do_not_learn(tmp_path):
    app, clock = make_app(tmp_path, wpm=200)
    async with app.run_test(size=(80, 24)) as pilot:
        clock.now += 0.5
        await pilot.press("space")  # skimmed
        clock.now += 30
        await pilot.press("j", "j")
        assert app.config.wpm == 200


async def test_time_spent_hidden_is_not_reading_time(tmp_path):
    app, clock = make_app(tmp_path, wpm=200)
    async with app.run_test(size=(80, 24)) as pilot:
        words = app._view_words
        await pilot.press("grave_accent")
        clock.now += 120  # someone walked by
        await pilot.press("grave_accent")
        clock.now += 15
        await pilot.press("space")
        assert app.config.wpm == learn(200, words, 15)


async def test_line_scrolling_keeps_the_clock_and_counts_the_new_lines(tmp_path):
    # a screen read for 20 s, a line scrolled, then paged 3 s later: 23 s of reading,
    # not a whole screen in 3 s
    app, clock = make_app(tmp_path, wpm=200)
    async with app.run_test(size=(80, 24)) as pilot:
        before = app._view_words
        clock.now += 20
        await pilot.press("j", "j")
        clock.now += 3
        read = app._view_words
        assert read >= before  # the lines scrolled in add up, nothing is lost
        await pilot.press("space")
        assert app.config.wpm == learn(200, read, 23)
