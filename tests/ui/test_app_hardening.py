from fcc.state import Config, load_progress
from fcc.ui.app import FccApp
from tests.ui.test_app_manual import make_txt


def cfg() -> Config:
    return Config(mask="code", lang="python", noise=0)


async def test_ctrl_p_opens_no_command_palette(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        base = app.screen
        await pilot.press("ctrl+p")
        await pilot.pause()
        assert app.screen is base
        assert len(app.screen_stack) == 1
        assert not app.panic


async def test_ctrl_q_saves_before_quitting(tmp_path):
    st = tmp_path / "st"
    st.mkdir()
    app = FccApp(make_txt(tmp_path), cfg(), st)
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("space", "space")
        pos = app.session.pos
        assert pos != (0, 0)
        book_id = app.session.book.id
        await pilot.press("ctrl+q")
        await pilot.pause()
    saved = load_progress(st)
    assert (saved[book_id].chapter, saved[book_id].paragraph) == pos


async def test_auto_end_switches_back_to_manual(tmp_path):
    app = FccApp(make_txt(tmp_path, n=3), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("a")
        for _ in range(200):
            app.tick()
        assert app.session.pos == (0, 2)
        assert not app.auto
        assert app._timer is None
        top = app.feed.visible_lines()[0]
        await pilot.press("k")  # manual keys work again
        assert app.feed.visible_lines()[0] != top
