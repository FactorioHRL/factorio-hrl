import pytest


def test_latest_resolves_to_the_newest_run_with_a_model(tmp_path):
    pytest.importorskip("stable_baselines3")
    from factorio_rl.train import resolve_init_from

    for name in ("cnn-x-p1__task__20260101-000000", "cnn-x-p1__task__20260102-000000", "cnn-x-p1__task__20260103-000000", "cnn-x-p2__task__20260104-000000"):
        (tmp_path / name).mkdir()
        if not name.endswith("20260103-000000"):  # the newest p1 run never finished: no best.zip
            (tmp_path / name / "best.zip").write_bytes(b"")
    assert resolve_init_from("latest:cnn-x-p1", str(tmp_path)).endswith("cnn-x-p1__task__20260102-000000/best.zip")
    assert resolve_init_from("runs/some/best.zip", str(tmp_path)) == "runs/some/best.zip"
    with pytest.raises(FileNotFoundError, match="run that phase first"):
        resolve_init_from("latest:cnn-y-p1", str(tmp_path))
