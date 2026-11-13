import pytest

from factorio_rl.config import TrackingConfig, TrainingConfig
from factorio_rl.tracking import Tracker, flatten

mlflow = pytest.importorskip("mlflow")


def test_flatten_makes_dotted_keys():
    assert flatten({"a": 1, "ppo": {"gamma": 0.9, "x": {"y": 2}}}) == {"a": 1, "ppo.gamma": 0.9, "ppo.x.y": 2}


def test_disabled_tracker_does_nothing(tmp_path):
    with Tracker(TrackingConfig(enabled=False), "e", "r", TrainingConfig(name="t")) as tracker:
        tracker.log({"x": 1.0}, 0)
        tracker.log_files(tmp_path / "missing")
    assert not list(tmp_path.iterdir())


def test_metrics_and_files_land_in_a_local_store(tmp_path):
    config = TrackingConfig(uri=f"sqlite:///{tmp_path}/mlflow.db", artifact_dir=str(tmp_path / "artifacts"))
    note = tmp_path / "best_blueprint.txt"
    note.write_text("0abc")
    with Tracker(config, "experiment", "run-1", TrainingConfig(name="t")) as tracker:
        tracker.log({"eval_mean_score": 0.5}, 1000)
        tracker.log({"eval_mean_score": 0.7}, 2000)
        tracker.log_files(note, tmp_path / "does-not-exist.zip")
    client = mlflow.MlflowClient(config.uri)
    (found,) = client.search_runs([client.get_experiment_by_name("experiment").experiment_id])
    assert found.data.params["ppo.gamma"] == "0.99"
    history = client.get_metric_history(found.info.run_id, "eval_mean_score")
    assert [(m.step, m.value) for m in history] == [(1000, 0.5), (2000, 0.7)]
    assert [a.path for a in client.list_artifacts(found.info.run_id)] == ["best_blueprint.txt"]
