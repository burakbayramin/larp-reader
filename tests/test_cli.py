import pytest

from fcc import cli
from fcc.state import Config, Entry, load_config, save_config, save_progress


@pytest.fixture(autouse=True)
def appdata(tmp_path, monkeypatch):
    monkeypatch.setenv("APPDATA", str(tmp_path / "appdata"))
    d = tmp_path / "appdata" / "fcc"
    d.mkdir(parents=True)
    return d


@pytest.fixture
def calls(monkeypatch):
    seen = []
    monkeypatch.setattr(cli, "run_app", lambda path, config, state_dir: seen.append((path, config, state_dir)))
    return seen


def forbidden(text: str):
    low = text.lower()
    return [w for w in ("book", "reader", "okuyucu") if w in low]


def test_version_has_no_forbidden_words(capsys):
    with pytest.raises(SystemExit) as e:
        cli.main(["--version"])
    assert e.value.code == 0
    out = capsys.readouterr().out
    assert out.startswith("fcc 0.4.2 (")
    assert forbidden(out) == []


def test_help_has_no_forbidden_words(capsys):
    with pytest.raises(SystemExit):
        cli.main(["--help"])
    out = capsys.readouterr().out
    assert "build" in out
    assert forbidden(out) == []
    with pytest.raises(SystemExit):
        cli.main(["build", "--help"])
    out = capsys.readouterr().out
    assert "--mask" in out
    assert forbidden(out) == []


def test_build_updates_config(tmp_path, calls, appdata):
    f = tmp_path / "a.txt"
    f.write_text("hello", encoding="utf-8")
    rc = cli.main(["build", str(f), "--mask=code", "--lang=go", "--noise=4", "--auto", "--speed=2.5"])
    assert rc == 0
    assert len(calls) == 1
    path, config, sdir = calls[0]
    assert path == f
    assert (config.mask, config.lang, config.noise, config.auto, config.speed) == ("code", "go", 4, True, 2.5)
    assert sdir == appdata
    assert load_config(appdata) == config


def test_build_without_flags_keeps_saved_config(tmp_path, calls, appdata):
    save_config(appdata, Config(mask="code", lang="java", noise=1))
    f = tmp_path / "a.txt"
    f.write_text("hello", encoding="utf-8")
    cli.main(["build", str(f)])
    assert calls[0][1].lang == "java" and calls[0][1].noise == 1


def test_no_args_uses_last_entry(tmp_path, calls, appdata):
    f = tmp_path / "a.txt"
    f.write_text("hello", encoding="utf-8")
    save_progress(appdata, {
        "x": Entry(str(tmp_path / "old.txt"), 0, 0, [], "2026-01-01T00:00:00"),
        "y": Entry(str(f), 0, 0, [], "2026-02-01T00:00:00"),
    })
    assert cli.main([]) == 0
    assert calls[0][0] == f


def test_no_args_without_history_errors(calls, capsys):
    assert cli.main([]) == 1
    assert calls == []
    assert "no input files" in capsys.readouterr().err


@pytest.mark.parametrize(
    "flag", ["--noise=9", "--noise=-1", "--speed=0", "--speed=-2", "--speed=abc", "--mask=nope", "--lang=cobol"]
)
def test_bad_flag_rejected_before_config_saved(tmp_path, calls, appdata, flag, capsys):
    f = tmp_path / "a.txt"
    f.write_text("hello", encoding="utf-8")
    with pytest.raises(SystemExit) as e:
        cli.main(["build", str(f), flag])
    assert e.value.code == 2
    assert calls == []
    assert not (appdata / "config.json").exists()


def test_noise_9_does_not_poison_later_runs(tmp_path, calls, appdata):
    f = tmp_path / "a.txt"
    f.write_text("hello", encoding="utf-8")
    with pytest.raises(SystemExit):
        cli.main(["build", str(f), "--noise", "9"])
    assert cli.main(["build", str(f)]) == 0
    assert calls[-1][1].noise == Config().noise
