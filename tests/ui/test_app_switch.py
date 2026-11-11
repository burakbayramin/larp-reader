from fcc.masks import MASK_NAMES
from fcc.state import load_config
from fcc.ui.app import FccApp
from tests.ui.test_app_auto_panic import book_words, shown
from fcc.state import Config
from tests.ui.test_app_manual import make_txt


def make_app(tmp_path, **kw):
    (tmp_path / "st").mkdir(exist_ok=True)
    config = Config(mask="code", lang="python", noise=kw.get("noise", 0))
    return FccApp(make_txt(tmp_path), config, tmp_path / "st")


async def test_m_cycles_masks_and_keeps_pos(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        await pilot.press("space")
        pos = app.session.pos
        assert pos != (0, 0)
        seen = [app._mask.name]
        for _ in range(len(MASK_NAMES)):
            await pilot.press("m")
            await pilot.pause()
            seen.append(app._mask.name)
            assert app._mask is app.session.mask
            assert app.session.pos == pos
            assert any(ln.pos == pos for ln in app.feed.visible_lines())
            assert load_config(tmp_path / "st").mask == app._mask.name
        assert seen == ["code", "log", "diff", "test", "build", "code"]


async def test_m_redraws_with_new_mask_style(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        await pilot.press("m")
        await pilot.pause()
        book = [ln.plain for ln in app.feed.visible_lines() if ln.is_book]
        assert book and all(" INFO " in t for t in book)
        assert "Tailing" in str(app.query_one("#status").render())


async def test_m_during_panic_switches_panic_stream_mask(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        await pilot.press("escape")
        await pilot.press("m")
        await pilot.pause()
        assert app.panic and app._mask.name == "log"
        for _ in range(10):
            app.tick()
        assert any(" svc." in ln.plain for ln in app.feed.history_lines())
        words = book_words(app.session.book)
        assert not any(w in shown(app) for w in words)


async def test_noise_keys_clamped(tmp_path):
    app = make_app(tmp_path, noise=4)
    async with app.run_test(size=(100, 24)) as pilot:
        await pilot.press("n")
        assert app.session.noise == 5
        await pilot.press("n")
        assert app.session.noise == 5
        assert load_config(tmp_path / "st").noise == 5
        for _ in range(7):
            await pilot.press("N")
        assert app.session.noise == 0
        assert load_config(tmp_path / "st").noise == 0


async def test_l_cycles_languages_and_keeps_pos(tmp_path):
    from fcc.masks.code import LANGS

    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        await pilot.press("space")
        pos = app.session.pos
        assert pos != (0, 0)
        start = LANGS.index(app._mask.lang)
        seen = []
        for _ in range(len(LANGS)):
            before = [ln.plain for ln in app.feed.visible_lines()]
            await pilot.press("l")
            await pilot.pause()
            seen.append(app._mask.lang)
            assert app._mask.name == "code" and app._mask is app.session.mask
            assert app.session.pos == pos
            assert any(ln.pos == pos for ln in app.feed.visible_lines())
            assert [ln.plain for ln in app.feed.visible_lines()] != before
            assert load_config(tmp_path / "st").lang == app._mask.lang
        assert seen == [LANGS[(start + i + 1) % len(LANGS)] for i in range(len(LANGS))]


async def test_shift_keys_cycle_backwards(tmp_path):
    from fcc.masks.code import LANGS

    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        lang = app._mask.lang
        await pilot.press("L")
        await pilot.pause()
        assert app._mask.lang == LANGS[LANGS.index(lang) - 1]
        await pilot.press("M")
        await pilot.pause()
        assert app._mask.name == MASK_NAMES[-1]
        assert load_config(tmp_path / "st").mask == MASK_NAMES[-1]


async def test_l_from_another_mask_returns_to_code_in_the_next_language(tmp_path):
    from fcc.masks.code import LANGS

    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        lang = app.config.lang
        await pilot.press("m")
        await pilot.pause()
        assert app._mask.name == "log"
        await pilot.press("l")
        await pilot.pause()
        assert app._mask.name == "code"
        assert app._mask.lang == LANGS[(LANGS.index(lang) + 1) % len(LANGS)]
        cfg = load_config(tmp_path / "st")
        assert (cfg.mask, cfg.lang) == ("code", app._mask.lang)


async def test_language_switch_during_panic_shows_no_text(tmp_path):
    app = make_app(tmp_path)
    async with app.run_test(size=(100, 24)) as pilot:
        await pilot.press("escape")
        await pilot.press("l")
        await pilot.pause()
        assert app.panic
        assert app.feed.history_lines()
        assert not any(ln.is_book for ln in app.feed.history_lines())
