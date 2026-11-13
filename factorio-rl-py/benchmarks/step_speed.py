"""Steps per second of ``LayoutEnv`` under random legal play (one process, one thread).

    python benchmarks/step_speed.py --dump ../data/data-raw-dump.json [--episodes 30]

Deterministic: the same seed plays the same episodes, so numbers before and after a change are
comparable. Also times the two engine calls every step makes, to show where the time goes.
"""

from __future__ import annotations

import argparse
import time
from pathlib import Path

import numpy as np

from factorio_rl import Engine, LayoutEnv, load_task_spec

TASKS = Path(__file__).parent.parent / "configs" / "tasks"


def episode(env: LayoutEnv, rng: np.random.Generator) -> int:
    env.reset(seed=int(rng.integers(1 << 30)))
    steps, done = 0, False
    while not done:
        legal = np.flatnonzero(env.action_masks()[:-1])
        if not len(legal):
            break
        _, _, terminated, truncated, _ = env.step(int(rng.choice(legal)))
        steps, done = steps + 1, terminated or truncated
    return steps


def bench(engine: Engine, task_file: str, episodes: int) -> None:
    spec = load_task_spec(TASKS / task_file)
    env = LayoutEnv(engine, spec)
    rng = np.random.default_rng(0)
    episode(env, rng)  # warm up caches
    rng = np.random.default_rng(0)
    start = time.perf_counter()
    steps = sum(episode(env, rng) for _ in range(episodes))
    elapsed = time.perf_counter() - start

    # the engine calls of one step, on the layout the last episode ended with
    task, layout = env.task, env.layout
    reps = 200
    t = time.perf_counter()
    for _ in range(reps):
        engine.evaluate(task, layout)
    evaluate_ms = (time.perf_counter() - t) / reps * 1e3
    t = time.perf_counter()
    for _ in range(reps):
        engine.legal_actions(task, layout)
    legal_ms = (time.perf_counter() - t) / reps * 1e3
    print(
        f"{task_file:<42} {steps / elapsed:>7.0f} steps/s   "
        f"(final layout: {len(layout)} machines; evaluate {evaluate_ms:.2f} ms, legal_actions {legal_ms:.2f} ms)"
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--dump", default=None, help="data-raw-dump.json (default: $FSIM_DUMP)")
    parser.add_argument("--episodes", type=int, default=30)
    args = parser.parse_args()
    engine = Engine(args.dump)
    for name in ("green_circuit.toml", "low_density_structure.toml", "production_science.toml"):
        bench(engine, name, args.episodes)
