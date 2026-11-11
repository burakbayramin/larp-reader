"""Manual paging must show every rendered line of the text, in order (spec success criterion 2)."""
from pathlib import Path

import pytest

from fcc.masks import MASK_NAMES
from fcc.state import Config
from fcc.ui.app import FccApp

MAX_PRESSES = 600


def make_book(tmp_path: Path, paras: int = 8, words: int = 60, name: str = "paging.txt") -> tuple[Path, list[str]]:
    blocks = [" ".join(f"p{p}w{w}" for w in range(words)) for p in range(paras)]
    path = tmp_path / name
    path.write_text("\n\n".join(blocks), encoding="utf-8")
    return path, " ".join(blocks).split()


def cfg(mask: str, noise: int) -> Config:
    return Config(mask=mask, lang="python", noise=noise)


def screen_words(app, width: int) -> list[str]:
    """Book words actually inside the visible window (rows and columns)."""
    out: list[str] = []
    for ln in app.feed.visible_lines():
        if not ln.is_book:
            continue
        assert len(ln.plain) <= width, ln.plain
        col = 0
        for text, style in ln.segments:
            if "book" in style.split():
                out += text[: max(0, width - col)].split()
            col += len(text)
    return out


def screen_key(app) -> list:
    return [(ln.plain, ln.pos) for ln in app.feed.visible_lines()]


async def collect(pilot, app, key: str, width: int) -> list[list[str]]:
    """Press `key` until the screen stops changing; return the book words of every screen seen."""
    screens = [screen_words(app, width)]
    last = screen_key(app)
    for _ in range(MAX_PRESSES):
        await pilot.press(key)
        cur = screen_key(app)
        if cur == last:
            return screens
        screens.append(screen_words(app, width))
        last = cur
    raise AssertionError(f"{key!r} never reached the end")


@pytest.mark.parametrize("noise", [0, 2])
@pytest.mark.parametrize("width", [60, 40])
@pytest.mark.parametrize("mask", MASK_NAMES)
async def test_space_paging_shows_every_word_once_in_order(tmp_path, request, mask, width, noise):
    if mask == "log" and width == 40:  # TODO(item 3): log prefix is wider than the pane
        request.applymarker(pytest.mark.xfail(strict=True, reason="log prefix wider than 40 cols"))
    path, words = make_book(tmp_path)
    app = FccApp(path, cfg(mask, noise), tmp_path / "st")
    async with app.run_test(size=(width, 24)) as pilot:
        screens = await collect(pilot, app, "space", width)
        seen = [w for s in screens for w in s]
        assert seen == words


@pytest.mark.parametrize("key", ["space", "j"])
async def test_paragraph_taller_than_screen_fully_readable(tmp_path, key):
    path, words = make_book(tmp_path, paras=1, words=400)
    app = FccApp(path, cfg("code", 0), tmp_path / "st")
    async with app.run_test(size=(60, 24)) as pilot:
        assert sum(ln.is_book for ln in app.session.paragraph_lines((0, 0))) > 2 * 23
        screens = await collect(pilot, app, key, 60)
        if key == "space":
            assert [w for s in screens for w in s] == words
        else:
            # line scrolling overlaps screens; every word must still appear, in order
            seen: list[str] = []
            for s in screens:
                for w in s:
                    if w not in seen:
                        seen.append(w)
            assert seen == words


async def test_j_and_k_scroll_one_line_across_paragraphs(tmp_path):
    path, _ = make_book(tmp_path, paras=4, words=60)
    app = FccApp(path, cfg("diff", 0), tmp_path / "st")
    async with app.run_test(size=(60, 24)) as pilot:
        s = app.session
        content = [ln for p in [(0, 0), (0, 1), (0, 2), (0, 3)] for ln in s.paragraph_lines(p)]
        first = len(s.paragraph_lines((0, 0)))
        header = len(s.mask.header(None))
        # at the very top the header precedes the text
        assert app.feed.visible_lines()[header:] == content[: 23 - header]
        for i in range(1, first + 2):
            await pilot.press("j")
            assert app.feed.visible_lines()[0] == content[i]
        assert s.pos == (0, 1)
        for i in range(first, 0, -1):
            await pilot.press("k")
            assert app.feed.visible_lines()[0] == content[i]
        assert s.pos == (0, 0)


async def test_k_at_top_and_space_at_end_do_nothing(tmp_path):
    path, _ = make_book(tmp_path, paras=2, words=20)
    app = FccApp(path, cfg("code", 0), tmp_path / "st")
    async with app.run_test(size=(80, 24)) as pilot:
        before = screen_key(app)
        await pilot.press("k", "up", "space", "j")  # everything already fits on one screen
        assert screen_key(app) == before
        assert app.session.pos == (0, 0)
