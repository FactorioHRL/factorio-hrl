"""Gymnasium environment: place assemblers on a grid to turn a supply of plates into circuits."""

from __future__ import annotations

from typing import Any

import gymnasium as gym
import numpy as np
from gymnasium import spaces

from .engine import Engine
from .scoring import Weights, breakdown, resolve_weights, score
from .config import TaskSpec
from .task import Placement, Task, ascii_grid

#: reward for an action that is not legal (only possible when the caller ignores `action_masks`)
ILLEGAL_PENALTY = -0.05


class LayoutEnv(gym.Env):
    """Builds a layout one machine at a time.

    * Task: sampled from the ``TaskSpec`` on every ``reset`` (or given as ``options={"task": Task}``):
      grid size and the supply per second. Recipes, machines and the target are fixed by the spec.
    * Action: ``Discrete(K * max_height * max_width + 1)``. ``a = (kind * max_height + y) * max_width + x``
      builds machine ``kind`` (see ``Engine.kinds``) with its top left tile at ``(x, y)``; the last
      action stops the episode. ``action_masks()`` marks the legal ones (fits the real grid, no
      overlap), which is what ``sb3_contrib.MaskablePPO`` consumes.
    * Observation: ``grid`` ``(K + 1, max_height, max_width)``: one occupancy plane per kind, then a
      plane marking the tiles of the real (possibly smaller) grid; ``task`` is a vector with the grid
      size, the supplies, the best possible output, the current output, waste and shape ratios, and per kind the number of
      machines built and their utilisation.
    * Reward: change of ``scoring.score`` (a weighted sum of terms: output, idle machines, and the
      shape of machines doing the same thing; see ``REWARDS.md``) after the action, so the episode
      return is the final score. ``reward`` overrides the default weights.
    """

    metadata = {"render_modes": ["ansi"]}

    def __init__(
        self,
        engine: Engine,
        spec: TaskSpec,
        reward: Weights | None = None,
        render_mode: str | None = None,
    ) -> None:
        super().__init__()
        self.engine = engine
        self.task_spec = spec
        self.max_width, self.max_height = spec.max_width, spec.max_height
        max_width, max_height = self.max_width, self.max_height
        self.supply_items = spec.supply_items
        self.max_supply = np.array([spec.supply[i].scale for i in self.supply_items], dtype=np.float32)
        self.weights = resolve_weights(reward)  # the reward terms and their weights, see scoring.py
        self.render_mode = render_mode

        probe = spec.sample(np.random.default_rng(0))
        self.kinds = engine.kinds(probe)
        self._cells = max_height * max_width
        self._kind_index = {(k.machine, k.recipe): i for i, k in enumerate(self.kinds)}
        self._max_machines = self._cells // min(k.width * k.height for k in self.kinds)
        self.stop_action = len(self.kinds) * self._cells
        self.action_space = spaces.Discrete(self.stop_action + 1)
        self.observation_space = spaces.Dict(
            {
                "grid": spaces.Box(0.0, 1.0, (len(self.kinds) + 1, max_height, max_width), np.float32),
                "task": spaces.Box(0.0, 1.0, (len(self.supply_items) + 11 + 2 * len(self.kinds),), np.float32),
            }
        )
        self.task: Task | None = None
        self.layout: list[Placement] = []
        self._report: dict = {}
        self._score = 0.0
        self._steps = 0
        self._max_steps = 0
        self._legal = np.zeros(self.stop_action + 1, dtype=bool)

    # -- gymnasium API ---------------------------------------------------------------------

    def reset(
        self, *, seed: int | None = None, options: dict[str, Any] | None = None
    ) -> tuple[dict[str, np.ndarray], dict]:
        super().reset(seed=seed)
        task = (options or {}).get("task") or self.task_spec.sample(self.np_random)
        if task.width > self.max_width or task.height > self.max_height:
            raise ValueError(f"task grid {task.width}x{task.height} exceeds the environment's maximum")
        self.task, self.layout = task, []
        self.episode = self.engine.start(task)
        self._report = self.episode.report()
        self._score = score(self._report, self.weights)
        self._steps = 0
        # every legal build adds a machine; the extra steps absorb a few illegal actions
        self._max_steps = self._report["tiles_total"] // min(k.width * k.height for k in self.kinds) + 5
        self._refresh_legal()
        return self._observation(), self._info()

    def step(self, action: int) -> tuple[dict[str, np.ndarray], float, bool, bool, dict]:
        assert self.task is not None, "call reset() first"
        self._steps += 1
        action = int(action)
        reward, terminated, illegal = 0.0, False, False
        if action == self.stop_action:
            terminated = True
        elif not self._legal[action]:
            reward, illegal = ILLEGAL_PENALTY, True
        else:
            kind_index, cell = divmod(action, self._cells)
            y, x = divmod(cell, self.max_width)
            kind = self.kinds[kind_index]
            self.episode.place(kind_index, x, y)
            self.layout.append(Placement(kind.machine, kind.recipe, x, y))
            self._report = self.episode.report()
            new_score = score(self._report, self.weights)
            reward, self._score = new_score - self._score, new_score
            self._refresh_legal()
            terminated = not self._legal[:-1].any()
        truncated = not terminated and self._steps >= self._max_steps
        return self._observation(), reward, terminated, truncated, self._info(illegal)

    def action_masks(self) -> np.ndarray:
        """Boolean mask over the action space (stopping is always legal)."""
        return self._legal.copy()

    def render(self) -> str | None:
        if self.render_mode != "ansi" or self.task is None:
            return None
        return ascii_grid(self.task.width, self.task.height, self._report["placed"])

    # -- helpers ---------------------------------------------------------------------------

    @property
    def report(self) -> dict:
        """The engine's report for the current layout."""
        return self._report

    def blueprint(self) -> str:
        """The current layout as a blueprint string (paste it into the game)."""
        return self.engine.export_blueprint(self.layout)

    def _refresh_legal(self) -> None:
        legal = self.episode.legal()  # [kind, y, x] on the real grid
        padded = np.zeros((len(self.kinds), self.max_height, self.max_width), dtype=bool)
        padded[:, : self.task.height, : self.task.width] = legal
        self._legal = np.append(padded.reshape(-1), True)

    def _observation(self) -> dict[str, np.ndarray]:
        grid = np.zeros((len(self.kinds) + 1, self.max_height, self.max_width), dtype=np.float32)
        grid[:-1, : self.task.height, : self.task.width] = self.episode.planes()
        grid[-1, : self.task.height, : self.task.width] = 1.0
        # unlimited supplies (None) are shown as the largest value
        supplies = np.array(
            [self.task.supply[i] if self.task.supply[i] is not None else self.max_supply[n] for n, i in enumerate(self.supply_items)],
            dtype=np.float32,
        )
        report = self._report
        # per kind: how many machines stand (share of what fits) and how busy they are, so the agent
        # can see which stage of a chain is the bottleneck and which idles
        built, busy = np.zeros(len(self.kinds), np.float32), np.zeros(len(self.kinds), np.float32)
        for group in report["groups"]:
            i = self._kind_index[(group["machine"], group["recipe"])]
            built[i], busy[i] = min(group["count"] / self._max_machines, 1.0), group["utilisation"]
        task = np.concatenate(
            [
                np.array([self.task.width / self.max_width, self.task.height / self.max_height], dtype=np.float32),
                np.clip(supplies / self.max_supply, 0.0, 1.0),
                np.array(
                    [
                        report["ceiling_per_second"] / (report["ceiling_per_second"] + 1.0),
                        report["output_ratio"],
                        report["waste_ratio"],
                        report["adjacency_ratio"],
                        report["line_ratio"],
                        report["stray_ratio"],
                        report["enclosed_ratio"],
                        report["cut_off_ratio"],
                        report["link_ratio"],
                    ],
                    dtype=np.float32,
                ),
                built,
                busy,
            ]
        )
        return {"grid": grid, "task": task.astype(np.float32)}

    def _info(self, illegal: bool = False) -> dict:
        r = self._report
        return {
            "task": self.task,
            "machines": r["machines"],
            "output_per_second": r["output_per_second"],
            "ceiling_per_second": r["ceiling_per_second"],
            "output_ratio": r["output_ratio"],
            "waste_ratio": r["waste_ratio"],
            "reward_terms": breakdown(r, self.weights),
            "score": self._score,
            "illegal": illegal,
        }
