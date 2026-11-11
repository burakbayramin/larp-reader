from pathlib import Path

from fcc.errors import EmptyBook
from fcc.state import Config, load_progress
from fcc.ui.app import FccApp


def make_txt(tmp_path: Path, n: int = 10) -> Path:
    p = tmp_path / "sample.txt"
    paras = [f"Paragraph number {i} " + "lorem ipsum dolor sit amet " * 8 for i in range(n)]
    p.write_text("\n\n".join(paras), encoding="utf-8")
    return p


def cfg() -> Config:
    return Config(mask="code", lang="python", noise=0)


def status_text(app) -> str:
    return str(app.query_one("#status").render())


async def test_manual_space_advances_and_status_updates(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        assert app.session.progress()[0] == 1
        await pilot.press("space")
        assert app.session.progress()[0] > 1
        assert "/10" in status_text(app)
        assert f" {app.session.progress()[0]}/10" in status_text(app)


async def test_space_advances_by_visible_paragraph_count(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        visible = {ln.pos for ln in app.feed.visible_lines() if ln.is_book}
        assert len(visible) >= 2
        await pilot.press("space")
        assert app.session.progress()[0] == 1 + len(visible)


async def test_down_and_j_advance_one_paragraph(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("down")
        assert app.session.progress()[0] == 2
        await pilot.press("j")
        assert app.session.progress()[0] == 3


async def test_up_goes_back_one_paragraph(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("down", "down")
        assert app.session.progress()[0] == 3
        await pilot.press("up")
        assert app.session.progress()[0] == 2
        await pilot.press("k")
        assert app.session.progress()[0] == 1


async def test_q_saves_progress(tmp_path):
    path = make_txt(tmp_path)
    st = tmp_path / "st"
    st.mkdir()
    app = FccApp(path, cfg(), st)
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("down", "down", "down")
        pos = app.session.pos
        book_id = app.session.book.id
        await pilot.press("q")
    saved = load_progress(st)
    assert (saved[book_id].chapter, saved[book_id].paragraph) == pos
    app2 = FccApp(path, cfg(), st)
    async with app2.run_test(size=(80, 24)):
        assert app2.session.pos == pos
        assert app2.session.progress()[0] == 4


async def test_resize_keeps_paragraph(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("down", "down")
        pos = app.session.pos
        await pilot.resize_terminal(50, 24)
        await pilot.pause()
        assert app.session.pos == pos
        assert app.session.width == 50
        lines = app.feed.visible_lines()
        assert lines
        assert all(len(ln.plain) <= 50 for ln in lines)
        assert any(ln.pos == pos for ln in lines)


async def test_narrow_terminal_shows_warning(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(38, 24)):
        lines = app.feed.visible_lines()
        warnings = [ln for ln in lines if "warning" in ln.plain]
        assert len(warnings) == 1
        assert all(len(ln.plain) <= 38 for ln in lines)


async def test_wide_terminal_has_no_warning(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)):
        assert not [ln for ln in app.feed.visible_lines() if "warning" in ln.plain]


async def test_empty_book_shows_error_not_crash(tmp_path):
    p = tmp_path / "empty.txt"
    p.write_text("", encoding="utf-8")
    app = FccApp(p, cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        assert app.session is None
        assert any(EmptyBook.title in ln.plain for ln in app.feed.visible_lines())
        await pilot.press("space", "down", "up")  # must not crash
        await pilot.press("q")


async def test_unregistered_mask_falls_back_to_code_with_error(tmp_path):
    app = FccApp(make_txt(tmp_path), Config(mask="build", lang="rust", noise=0), tmp_path / "st")
    async with app.run_test(size=(80, 24)):
        assert app.session is not None
        assert app.session.mask.name == "code"
        assert any(ln.plain.startswith("error:") for ln in app.feed.history_lines())
        assert any(ln.is_book for ln in app.feed.visible_lines())


async def test_marker_not_rendered_and_no_forbidden_words(tmp_path):
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)):
        rendered = app.feed.render_text()
        assert rendered.spans
        for span in rendered.spans:
            assert "book" not in str(span.style).lower()
        screen = rendered.plain + status_text(app) + app.title + str(app.sub_title)
        for word in ("book", "reader", "okuyucu"):
            assert word not in screen.lower()
