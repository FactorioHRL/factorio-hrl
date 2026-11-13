"""MLflow tracking, kept small on purpose: parameters once, a few metrics per evaluation and a handful
of files at the end. Nothing per training step, so a run costs kilobytes, not gigabytes.

``Tracker`` does nothing when tracking is disabled, so callers never branch on it.
Browse the runs with ``mlflow ui --backend-store-uri sqlite:///runs/mlflow.db`` (the default store).
"""

from __future__ import annotations

import dataclasses
from pathlib import Path
from typing import Any

from .config import TrackingConfig


def flatten(values: dict[str, Any], prefix: str = "") -> dict[str, Any]:
    """``{"ppo": {"gamma": 1}}`` -> ``{"ppo.gamma": 1}`` (MLflow parameters are flat)."""
    flat: dict[str, Any] = {}
    for key, value in values.items():
        if isinstance(value, dict):
            flat.update(flatten(value, f"{prefix}{key}."))
        else:
            flat[f"{prefix}{key}"] = value
    return flat


class Tracker:
    """One MLflow run. Use as a context manager."""

    def __init__(self, config: TrackingConfig, experiment: str, run_name: str, params: Any) -> None:
        self.config = config
        self._mlflow = None
        if not config.enabled:
            return
        try:
            import mlflow
        except ImportError as error:
            raise RuntimeError("tracking needs mlflow: pip install 'factorio-rl[train]' (or set tracking.enabled = false)") from error
        mlflow.set_tracking_uri(config.uri)
        if mlflow.get_experiment_by_name(experiment) is None:
            mlflow.create_experiment(experiment, artifact_location=Path(config.artifact_dir).resolve().as_uri())
        mlflow.set_experiment(experiment)
        mlflow.start_run(run_name=run_name)
        mlflow.log_params(flatten(dataclasses.asdict(params) if dataclasses.is_dataclass(params) else dict(params)))
        self._mlflow = mlflow

    def log(self, metrics: dict[str, float], step: int) -> None:
        """Logs scalar metrics at ``step`` (call it once per evaluation, not per training step)."""
        if self._mlflow:
            self._mlflow.log_metrics(metrics, step=step)

    def log_files(self, *paths: Path) -> None:
        """Uploads the files that exist (small ones: the best blueprint, the config copies, the model)."""
        if self._mlflow:
            for path in paths:
                if path.is_file():
                    self._mlflow.log_artifact(str(path))

    def close(self) -> None:
        if self._mlflow:
            self._mlflow.end_run()
            self._mlflow = None

    def __enter__(self) -> "Tracker":
        return self

    def __exit__(self, *exc) -> None:
        self.close()
