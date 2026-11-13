"""Python face of the Rust engine: validation and scoring of machine layouts."""

from __future__ import annotations

import json
import os
from pathlib import Path

import numpy as np

from . import _native
from .task import Kind, Placement, Task, layout_json

DUMP_ENV = "FSIM_DUMP"


class Episode:
    """A layout being built for one task. Unlike ``Engine.evaluate`` it keeps the layout on the Rust
    side, so a step sends only the new machine and gets the mask and occupancy back incrementally."""

    def __init__(self, native: _native.Episode, task: Task, kinds: list[Kind]) -> None:
        self._native, self.task, self.kinds = native, task, kinds
        self._shape = (len(kinds), task.height, task.width)

    def place(self, kind: int, x: int, y: int) -> None:
        """Builds machine ``kind`` with its top left tile at ``(x, y)``; ``ValueError`` if it does not fit."""
        self._native.place(kind, x, y)

    def legal(self) -> np.ndarray:
        """Boolean ``[kind, y, x]``: whether a machine of that kind still fits there."""
        return np.frombuffer(self._native.legal(), dtype=np.uint8).reshape(self._shape).astype(bool)

    def planes(self) -> np.ndarray:
        """Float32 ``[kind, y, x]``: 1 where a machine of that kind stands."""
        return np.frombuffer(self._native.planes(), dtype=np.uint8).reshape(self._shape).astype(np.float32)

    def report(self) -> dict:
        """The full report for the current layout."""
        return json.loads(self._native.report())


class Engine:
    """Game data plus the rules of the layout task. Loading takes ~0.2 s, so create one and reuse it.

    ``dump_path`` is a ``data-raw-dump.json`` from a vanilla game (default: ``$FSIM_DUMP``).
    """

    def __init__(self, dump_path: str | os.PathLike[str] | None = None) -> None:
        dump_path = dump_path or os.environ.get(DUMP_ENV)
        if not dump_path:
            raise ValueError(f"pass a data dump path or set ${DUMP_ENV}")
        if not Path(dump_path).is_file():
            raise FileNotFoundError(f"data dump not found: {dump_path}")
        self._native = _native.Engine(str(dump_path))

    def kinds(self, task: Task) -> list[Kind]:
        """The things an agent can build, in a stable order (their index is the action id)."""
        return [Kind(**k) for k in json.loads(self._native.kinds(task.to_json()))]

    def start(self, task: Task) -> Episode:
        """An empty layout for ``task``: the fast way to build one machine at a time."""
        return Episode(self._native.start(task.to_json()), task, self.kinds(task))

    def evaluate(self, task: Task, layout: list[Placement]) -> dict:
        """Validates a layout and returns the full report (see ``fsim_env::Report``)."""
        return json.loads(self._native.evaluate(task.to_json(), layout_json(layout)))

    def legal_actions(self, task: Task, layout: list[Placement]) -> np.ndarray:
        """Boolean array ``[kind, y, x]``: whether a machine of that kind fits there."""
        flat = np.frombuffer(
            self._native.legal_actions(task.to_json(), layout_json(layout)), dtype=np.uint8
        )
        return flat.reshape(len(self.kinds(task)), task.height, task.width).astype(bool)

    def analyze_blueprint(self, task: Task, blueprint: str) -> dict:
        """Report for the machines of a blueprint string; other entities become violations."""
        return json.loads(self._native.analyze_blueprint(task.to_json(), blueprint))

    def render_png(
        self,
        layout: list[Placement],
        game_dir: str | os.PathLike[str] | None = None,
        scale: int = 32,
    ) -> bytes:
        """PNG of a layout from the Rust renderer: real sprites when ``game_dir`` holds the game
        graphics (``data/game`` from ``scripts/setup-data.sh``), schematic boxes otherwise."""
        return bytes(self._native.render_png(layout_json(layout), str(game_dir) if game_dir else None, scale))

    def export_blueprint(self, layout: list[Placement]) -> str:
        """Blueprint string (paste it into the game) for a layout."""
        return self._native.export_blueprint(layout_json(layout))


def decode_blueprint(blueprint: str) -> dict:
    """Full decoded JSON of a blueprint string, as the game stores it."""
    return json.loads(_native.decode_blueprint(blueprint))
