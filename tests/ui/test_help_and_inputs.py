"""Esc shows the keys and the folder's files at the side; o/O and 1-9 switch file."""

from pathlib import Path

from fcc.state import Config, load_progress
from fcc.ui.app import FccApp


def make_folder(tmp_path: Path) -> Path:
    folder = tmp_path / "shelf"
    folder.mkdir()
    for name in ("alpha", "beta", "gamma"):
        paras = [f"{name} paragraph {i} " + "lorem ipsum dolor sit amet " * 8 for i in range(10)]
        (folder / f"{name}.txt").write_text("\n\n".join(paras), encoding="utf-8")
    (folder / "cover.jpg").write_bytes(b"\xff\xd8\xff")
    (folder / "notes").mkdir()
    return folder


def make_app(tmp_path: Path, name: str = "beta") -> FccApp:
    (tmp_path / "st").mkdir(exist_ok=True)
    folder = tmp_path / "shelf"
    if not folder.exists():
        make_folder(tmp_path)
    return FccApp(folder / f"{name}.txt", Config(mask="code", lang="python", noise=0), tmp_path / "st")


def panel(app) -> str:
    return str(app.query_one("#help").render())


def text_on_screen(app) -> str:
    return " ".join(ln.plain for ln in app.feed.visible_lines())


async def test_esc_toggles_a_side_panel_with_keys_and_files(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 30)) as pilot:
        help_ = app.query_one("#help")
        assert not help_.display
        before = [ln.plain for ln in app.feed.visible_lines()]
        await pilot.press("escape")
        await pilot.pause()
        assert help_.display and not app.panic
        shown = panel(app)
        for key in ("space", "m / M", "l / L", "n / N", "o / O", "1-9", "esc", "q"):
            assert key in shown, key
        assert "alpha.txt" in shown and "beta.txt" in shown and "gamma.txt" in shown
        assert "cover.jpg" not in shown and "notes" not in shown
        # the panel lies over the text; nothing is re-wrapped or moved
        assert [ln.plain for ln in app.feed.visible_lines()] == before
        assert help_.region.x >= 100 - help_.region.width and help_.region.width < 60
        await pilot.press("escape")
        await pilot.pause()
        assert not help_.display


async def test_panel_marks_the_open_file(tmp_path):
    app = make_app(tmp_path, "beta")
    async with app.run_test(size=(100, 30)) as pilot:
        await pilot.press("escape")
        await pilot.pause()
        rows = [r for r in panel(app).splitlines() if ".txt" in r]
        assert [r.strip()[0] for r in rows] == ["1", ">", "3"] or [">" in r for r in rows] == [False, True, False]


async def test_backtick_hides_everything_and_esc_does_not_undo_it(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 30)) as pilot:
        await pilot.press("escape")
        await pilot.press("grave_accent")
        await pilot.pause()
        assert app.panic and not app.query_one("#help").display
        assert not any(ln.is_book for ln in app.feed.history_lines())
        await pilot.press("escape")
        await pilot.pause()
        assert app.panic and not app.query_one("#help").display
        await pilot.press("grave_accent")
        await pilot.pause()
        assert not app.panic and "beta paragraph" in text_on_screen(app)


async def test_o_and_shift_o_walk_the_folder_and_keep_each_place(tmp_path):
    app = make_app(tmp_path, "alpha")
    async with app.run_test(size=(100, 30)) as pilot:
        await pilot.press("space")
        left_at = app.session.pos
        assert left_at != (0, 0)
        await pilot.press("o")
        await pilot.pause()
        assert "beta paragraph 0" in text_on_screen(app)
        await pilot.press("o")
        await pilot.pause()
        assert "gamma paragraph 0" in text_on_screen(app)
        await pilot.press("o")  # wraps round
        await pilot.pause()
        assert "alpha paragraph" in text_on_screen(app)
        assert app.session.pos == left_at
        await pilot.press("O")
        await pilot.pause()
        assert "gamma paragraph 0" in text_on_screen(app)
        assert len(load_progress(tmp_path / "st")) >= 2


async def test_digits_open_a_listed_file_only_while_the_panel_is_open(tmp_path):
    app = make_app(tmp_path, "alpha")
    async with app.run_test(size=(100, 30)) as pilot:
        await pilot.press("3")
        await pilot.pause()
        assert "alpha paragraph 0" in text_on_screen(app)
        await pilot.press("escape", "3")
        await pilot.pause()
        assert "gamma paragraph 0" in text_on_screen(app)
        assert ">" in [r for r in panel(app).splitlines() if "gamma.txt" in r][0]
        await pilot.press("9")  # no ninth file: nothing happens
        await pilot.pause()
        assert "gamma paragraph 0" in text_on_screen(app)


async def test_switching_file_is_ignored_while_hidden(tmp_path):
    app = make_app(tmp_path, "alpha")
    async with app.run_test(size=(100, 30)) as pilot:
        await pilot.press("grave_accent", "o")
        await pilot.pause()
        assert app.panic
        await pilot.press("grave_accent")
        await pilot.pause()
        assert "alpha paragraph 0" in text_on_screen(app)


async def test_one_file_in_the_folder_is_fine(tmp_path):
    (tmp_path / "st").mkdir()
    solo = tmp_path / "solo.txt"
    solo.write_text("only paragraph here " * 5, encoding="utf-8")
    app = FccApp(solo, Config(mask="code", lang="python", noise=0), tmp_path / "st")
    async with app.run_test(size=(100, 30)) as pilot:
        await pilot.press("o", "O", "escape")
        await pilot.pause()
        assert "only paragraph" in text_on_screen(app)
        assert "solo.txt" in panel(app)
