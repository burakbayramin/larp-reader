import json
import os
from pathlib import Path
from unittest import mock

from fcc import state
from fcc.state import (
    Config, Entry, last_entry, load_config, load_progress,
    save_config, save_progress, state_dir,
)


def test_roundtrip_config(tmp_path):
    cfg = Config(mask="log", lang="go", noise=0, speed=3.0, auto=True)
    save_config(tmp_path, cfg)
    assert load_config(tmp_path) == cfg


def test_missing_config_returns_defaults_without_bak(tmp_path):
    assert load_config(tmp_path) == Config()
    assert Config() == Config("build", "rust", 2, 1.5, False)
    assert list(tmp_path.iterdir()) == []


def test_config_ignores_unknown_and_fills_missing_keys(tmp_path):
    (tmp_path / "config.json").write_text(
        json.dumps({"mask": "log", "future_key": 1}), encoding="utf-8")
    cfg = load_config(tmp_path)
    assert cfg == Config(mask="log")


def test_roundtrip_progress_with_turkish_path(tmp_path):
    p = str(tmp_path / "Kitaplar" / "Suç ve Ceza.epub")
    data = {p: Entry(p, 3, 7, [[1, 2], [3, 4]], "2026-11-11T10:00:00")}
    save_progress(tmp_path, data)
    assert load_progress(tmp_path) == data
    raw = (tmp_path / "progress.json").read_text(encoding="utf-8")
    assert "Suç ve Ceza" in raw  # ensure_ascii=False


def test_corrupt_json_backed_up_and_defaults_returned(tmp_path):
    (tmp_path / "config.json").write_text("{not json", encoding="utf-8")
    (tmp_path / "progress.json").write_text("{not json", encoding="utf-8")
    assert load_config(tmp_path) == Config()
    assert load_progress(tmp_path) == {}
    assert (tmp_path / "config.json.bak").read_text(encoding="utf-8") == "{not json"
    assert (tmp_path / "progress.json.bak").exists()
    assert not (tmp_path / "config.json").exists()


def test_write_is_atomic(tmp_path):
    with mock.patch.object(state.os, "replace", wraps=os.replace) as rep:
        save_config(tmp_path, Config())
    assert rep.call_count == 1
    src, dst = rep.call_args.args
    assert Path(dst) == tmp_path / "config.json"
    assert Path(src).parent == tmp_path and Path(src) != Path(dst)
    assert [p.name for p in tmp_path.iterdir()] == ["config.json"]


def test_last_entry_picks_newest():
    a = Entry("a", 0, 0, [], "2026-01-01T00:00:00")
    b = Entry("b", 1, 1, [], "2026-06-01T00:00:00")
    c = Entry("c", 2, 2, [], "2026-03-01T00:00:00")
    assert last_entry({"a": a, "b": b, "c": c}) == ("b", b)
    assert last_entry({}) is None


def test_state_dir_uses_appdata(monkeypatch, tmp_path):
    monkeypatch.setenv("APPDATA", str(tmp_path))
    d = state_dir()
    assert d == tmp_path / "fcc"
    assert d.is_dir()


def test_invalid_utf8_backed_up_and_defaults_returned(tmp_path):
    (tmp_path / "config.json").write_bytes(b"\xff\xfe\x00")
    (tmp_path / "progress.json").write_bytes(b"\xff\xfe\x00")
    assert load_config(tmp_path) == Config()
    assert load_progress(tmp_path) == {}
    assert (tmp_path / "config.json.bak").read_bytes() == b"\xff\xfe\x00"
    assert (tmp_path / "progress.json.bak").read_bytes() == b"\xff\xfe\x00"
    assert not (tmp_path / "config.json").exists()
    assert not (tmp_path / "progress.json").exists()


import pytest  # noqa: E402


def _config_with(tmp_path, **fields_):
    (tmp_path / "config.json").write_text(json.dumps(fields_), encoding="utf-8")
    return load_config(tmp_path)


@pytest.mark.parametrize(
    "raw, expected",
    [
        (9, 5), (-3, 0), ("2", 2), ("4.0", 4), (3.7, 3), ("lots", Config().noise),
        (None, Config().noise), (True, Config().noise), ([1], Config().noise),
    ],
)
def test_config_noise_coerced_and_clamped(tmp_path, raw, expected):
    assert _config_with(tmp_path, noise=raw).noise == expected


@pytest.mark.parametrize(
    "raw, expected",
    [
        (100, 20.0), (0.01, 0.2), ("3", 3.0), (2, 2.0), (0, Config().speed), (-1, Config().speed),
        ("fast", Config().speed), (None, Config().speed), (float("nan"), Config().speed),
        (True, Config().speed),
    ],
)
def test_config_speed_coerced_and_clamped(tmp_path, raw, expected):
    assert _config_with(tmp_path, speed=raw).speed == expected


def test_config_unknown_mask_and_lang_fall_back_to_defaults(tmp_path):
    cfg = _config_with(tmp_path, mask="nope", lang="cobol", noise=1)
    assert (cfg.mask, cfg.lang, cfg.noise) == (Config().mask, Config().lang, 1)
    cfg = _config_with(tmp_path, mask=3, lang=None)
    assert (cfg.mask, cfg.lang) == (Config().mask, Config().lang)


def test_config_valid_values_kept(tmp_path):
    cfg = _config_with(tmp_path, mask="log", lang="go", noise=0, speed=0.2, auto=True)
    assert cfg == Config(mask="log", lang="go", noise=0, speed=0.2, auto=True)


def test_config_auto_must_be_bool(tmp_path):
    assert _config_with(tmp_path, auto="yes").auto is False
    assert _config_with(tmp_path, auto=1).auto is False


def test_progress_parsed_per_entry_and_not_quarantined(tmp_path):
    good = {"path": "C:/a.epub", "chapter": 1, "paragraph": 2, "bookmarks": [[0, 1]], "updated_at": "2026-11-01T10:00:00"}
    raw = {
        "good": good,
        "extra": dict(good, future_field="x"),
        "nomarks": {"path": "C:/b.epub", "chapter": 0, "paragraph": 5, "updated_at": "2026-11-02T10:00:00"},
        "nodate": {"path": "C:/c.epub", "chapter": 0, "paragraph": 1},
        "nochapter": {"path": "C:/d.epub", "paragraph": 1, "bookmarks": [], "updated_at": "x"},
        "badtype": dict(good, chapter="one"),
        "notdict": [1, 2, 3],
        "badmarks": dict(good, bookmarks=[[1, 2], "zz", [3], [True, 1], [4, 5]]),
    }
    (tmp_path / "progress.json").write_text(json.dumps(raw), encoding="utf-8")
    data = load_progress(tmp_path)
    assert not (tmp_path / "progress.json.bak").exists()
    assert set(data) == {"good", "extra", "nomarks", "nodate", "badmarks"}
    assert data["good"] == Entry(**good)
    assert data["extra"] == Entry(**good)
    assert data["nomarks"].bookmarks == []
    assert data["nodate"].updated_at == ""
    assert data["badmarks"].bookmarks == [[1, 2], [4, 5]]
    # saving keeps every readable book's progress
    save_progress(tmp_path, data)
    assert set(load_progress(tmp_path)) == set(data)


def test_progress_top_level_not_object_is_quarantined(tmp_path):
    (tmp_path / "progress.json").write_text("[1, 2]", encoding="utf-8")
    assert load_progress(tmp_path) == {}
    assert (tmp_path / "progress.json.bak").exists()
