"""Run configuration, read from TOML: a *task* file (what to build) and a *training* file (how to learn).

Task file: see ``configs/tasks``. Training file: see ``configs/training``.
"""

from __future__ import annotations

import tomllib
import warnings
from dataclasses import dataclass, field, fields
from pathlib import Path
from typing import Any

import numpy as np

from .models import ARCHITECTURES
from .scoring import resolve_weights
from .task import Task

MODES = ("supply_limited", "grid_limited", "target_output")
UNLIMITED = "unlimited"


def _read(path: str | Path) -> dict[str, Any]:
    with open(path, "rb") as f:
        return tomllib.load(f)


def _build(cls, section: dict[str, Any], where: str):
    """``cls(**section)`` with a readable error for unknown keys."""
    allowed = {f.name for f in fields(cls)}
    unknown = set(section) - allowed
    if unknown:
        raise ValueError(f"{where}: unknown keys {sorted(unknown)} (allowed: {sorted(allowed)})")
    return cls(**section)


# -- task ---------------------------------------------------------------------------------


@dataclass(frozen=True)
class Supply:
    """Rate of one input item: fixed, sampled from a whole-number range, or unlimited."""

    fixed: float | None = None
    low: int | None = None
    high: int | None = None
    unlimited: bool = False

    @classmethod
    def parse(cls, value: Any, item: str) -> Supply:
        if value == UNLIMITED:
            return cls(unlimited=True)
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            return cls(fixed=float(value))
        if isinstance(value, dict) and set(value) == {"min", "max"} and 0 <= value["min"] <= value["max"]:
            return cls(low=int(value["min"]), high=int(value["max"]))
        raise ValueError(f"supply.{item}: expected a number, {{ min, max }} or \"{UNLIMITED}\", got {value!r}")

    def sample(self, rng: np.random.Generator) -> float | None:
        """The rate for one episode; ``None`` means unlimited."""
        if self.unlimited:
            return None
        if self.fixed is not None:
            return self.fixed
        return float(rng.integers(self.low, self.high + 1))

    @property
    def scale(self) -> float:
        """Largest value the rate can take (1.0 for unlimited), used to normalise observations."""
        if self.unlimited:
            return 1.0
        return max(self.fixed if self.fixed is not None else self.high, 1.0)


@dataclass(frozen=True)
class TaskSpec:
    """Everything that defines a family of tasks; ``sample`` draws one concrete ``Task``."""

    name: str
    mode: str
    target: str
    recipes: tuple[str, ...]
    machines: tuple[str, ...]
    width_range: tuple[int, int]
    height_range: tuple[int, int]
    supply: dict[str, Supply]
    #: output per second to reach (``target_output`` mode only)
    required_output: Supply | None = None
    #: see ``Task.require_access``
    require_access: bool = False

    def __post_init__(self) -> None:
        if self.mode not in MODES:
            raise ValueError(f"mode must be one of {MODES}, got {self.mode!r}")
        unlimited = [s.unlimited for s in self.supply.values()]
        if self.mode in ("grid_limited", "target_output") and not all(unlimited):
            raise ValueError(f"{self.mode} tasks need every supply to be \"unlimited\"")
        if (self.mode == "target_output") != (self.required_output is not None):
            raise ValueError("[output] per_second is required by target_output tasks and only by them")
        if self.mode == "supply_limited" and all(unlimited):
            raise ValueError("supply_limited tasks need at least one limited supply (or use grid_limited)")

    @property
    def max_width(self) -> int:
        return self.width_range[1]

    @property
    def max_height(self) -> int:
        return self.height_range[1]

    @property
    def supply_items(self) -> tuple[str, ...]:
        return tuple(self.supply)

    def sample(self, rng: np.random.Generator) -> Task:
        return Task(
            width=int(rng.integers(self.width_range[0], self.width_range[1] + 1)),
            height=int(rng.integers(self.height_range[0], self.height_range[1] + 1)),
            supply={item: spec.sample(rng) for item, spec in self.supply.items()},
            recipes=self.recipes,
            machines=self.machines,
            target=self.target,
            required_output=self.required_output.sample(rng) if self.required_output else None,
            require_access=self.require_access,
        )


def _parse_output(section: dict[str, Any] | None, path: str | Path) -> Supply | None:
    """``[output] per_second = 4`` or ``{ min = 2, max = 6 }``."""
    if section is None:
        return None
    if set(section) != {"per_second"}:
        raise ValueError(f"{path}: [output] takes only per_second, got {sorted(section)}")
    required = Supply.parse(section["per_second"], "output.per_second")
    if required.unlimited or (required.fixed or required.low or 0) <= 0:
        raise ValueError(f"{path}: output.per_second must be a positive number or {{ min, max }}")
    return required


def load_task_spec(path: str | Path) -> TaskSpec:
    raw = _read(path)
    grid = raw.pop("grid", {})
    supply = raw.pop("supply", {})
    output = raw.pop("output", None)
    if not supply:
        raise ValueError(f"{path}: [supply] must list at least one input item")
    if "width" in grid or "height" in grid:
        width, height = (grid["width"],) * 2 if "height" not in grid else (grid["width"], grid["height"])
        ranges = ((width, width), (height, height))
    else:
        ranges = ((grid["min"], grid["max"]),) * 2
    return _build(
        TaskSpec,
        {
            **raw,
            "recipes": tuple(raw.get("recipes", ())),
            "machines": tuple(raw.get("machines", ())),
            "width_range": ranges[0],
            "height_range": ranges[1],
            "supply": {item: Supply.parse(v, item) for item, v in supply.items()},
            "required_output": _parse_output(output, path),
        },
        str(path),
    )


# -- training -----------------------------------------------------------------------------


@dataclass(frozen=True)
class PPOConfig:
    #: a ``best.zip`` of an earlier run to continue from (same observation and network); empty: start fresh
    init_from: str = ""
    total_timesteps: int = 500_000
    n_envs: int = 4
    n_steps: int = 512
    n_epochs: int = 10
    batch_size: int = 256
    learning_rate: float = 3e-4
    gamma: float = 0.99
    ent_coef: float = 0.01


@dataclass(frozen=True)
class GeneticConfig:
    population: int = 60
    generations: int = 200


@dataclass(frozen=True)
class EvaluationConfig:
    every: int = 20_000
    episodes: int = 20


@dataclass(frozen=True)
class StopConfig:
    target_vs_expert: float = 0.98
    patience: int = 8


@dataclass(frozen=True)
class LiveConfig:
    enabled: bool = False
    port: int = 8000
    game_dir: str = ""


@dataclass(frozen=True)
class TrackingConfig:
    """MLflow: what is stored is deliberately small (see ``tracking.py``)."""

    enabled: bool = True
    uri: str = "sqlite:///runs/mlflow.db"
    artifact_dir: str = "runs/mlartifacts"
    #: also copy the best model into the MLflow store when the run ends (it is already in the run
    #: directory and is 10-30 MB, so this doubles the disk use; off by default)
    log_model: bool = False


@dataclass(frozen=True)
class ModelConfig:
    """The network of the neural algorithms; see ``models/__init__.py``."""

    architecture: str = "mlp"
    #: options of the architecture (checked when the model is built); empty: its defaults
    options: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.architecture not in ARCHITECTURES:
            raise ValueError(f"model.architecture must be one of {list(ARCHITECTURES)}, got {self.architecture!r}")


ALGORITHMS = ("maskable_ppo", "genetic")


@dataclass(frozen=True)
class TrainingConfig:
    name: str
    algorithm: str = "maskable_ppo"
    seed: int = 0
    device: str = "auto"
    out_dir: str = "runs"
    ppo: PPOConfig = field(default_factory=PPOConfig)
    genetic: GeneticConfig = field(default_factory=GeneticConfig)
    evaluation: EvaluationConfig = field(default_factory=EvaluationConfig)
    stop: StopConfig = field(default_factory=StopConfig)
    live: LiveConfig = field(default_factory=LiveConfig)
    tracking: TrackingConfig = field(default_factory=TrackingConfig)
    #: reward-term weights that differ from the defaults, e.g. ``{"line": 0.1}`` (see ``scoring.py``)
    reward: dict[str, float] = field(default_factory=dict)
    model: ModelConfig = field(default_factory=ModelConfig)

    def __post_init__(self) -> None:
        resolve_weights(self.reward)  # unknown term names fail here, with the list of known ones
        if self.algorithm not in ALGORITHMS:
            raise ValueError(f"algorithm must be one of {ALGORITHMS}, got {self.algorithm!r}")


def load_training(path: str | Path) -> TrainingConfig:
    raw = _read(path)
    if "policy" in raw.get("ppo", {}):
        warnings.warn(f"{path}: [ppo] policy is ignored; the network is chosen in [model] architecture", stacklevel=2)
        del raw["ppo"]["policy"]
    sections = {"ppo": PPOConfig, "genetic": GeneticConfig, "evaluation": EvaluationConfig, "stop": StopConfig, "live": LiveConfig, "tracking": TrackingConfig, "model": ModelConfig}
    for key, cls in sections.items():
        if key in raw:
            raw[key] = _build(cls, raw[key], f"{path} [{key}]")
    return _build(TrainingConfig, raw, str(path))
