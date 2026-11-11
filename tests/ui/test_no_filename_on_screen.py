"""No part of the input file's name may reach the screen (spec success criterion 1)."""
import pytest
from textual.widgets import Input

from fcc.state import Config
from fcc.ui.app import FccApp
from tests.ui.test_app_manual import make_txt, status_text

NAME_PARTS = ("suç", "suc", "ceza", "epub")


def missing(tmp_path):
    return tmp_path / "Suç ve Ceza.epub"


def extensionless(tmp_path):
    p = tmp_path / "SucVeCeza"
    p.write_bytes(b"\x00\x01 just some opaque bytes \x02")
    return p


def screen(app) -> str:
    lines = app.feed.history_lines() + app.feed.visible_lines()
    return ("\n".join(ln.plain for ln in lines) + "\n" + status_text(app)).lower()


def cfg() -> Config:
    return Config(mask="code", lang="python", noise=0)


@pytest.mark.parametrize("make", [missing, extensionless])
async def test_bad_input_at_startup_hides_file_name(tmp_path, make):
    app = FccApp(make(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)):
        text = screen(app)
        assert "error:" in text
        assert not [p for p in NAME_PARTS if p in text], text


@pytest.mark.parametrize("make", [missing, extensionless])
async def test_bad_input_from_menu_hides_file_name(tmp_path, make):
    (tmp_path / "st").mkdir()
    app = FccApp(make_txt(tmp_path), cfg(), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.press("colon")
        await pilot.pause()
        field = app.screen.query_one("#path", Input)
        field.focus()
        field.value = str(make(tmp_path))
        await pilot.press("enter")
        await pilot.pause()
        text = screen(app)
        assert "error:" in text
        assert not [p for p in NAME_PARTS if p in text], text


def test_os_error_message_drops_file_name():
    from fcc.errors import InputUnavailable

    e = PermissionError(13, "Permission denied", r"C:\kitaplar\Suç ve Ceza.epub")
    msg = str(InputUnavailable.from_os_error(e)).lower()
    assert "permission denied" in msg
    assert not [p for p in NAME_PARTS if p in msg]


def test_extensionless_unsupported_uses_placeholder(tmp_path):
    from fcc.errors import UnsupportedFormat
    from fcc.loaders import load_book

    with pytest.raises(UnsupportedFormat) as e:
        load_book(extensionless(tmp_path))
    assert "(uzantısız)" in str(e.value)
    assert "sucveceza" not in str(e.value).lower()
