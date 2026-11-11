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
