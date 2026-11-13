"""Reference layouts: a brute-force expert (the score an agent should reach) and a genetic search."""

from __future__ import annotations

from collections.abc import Callable

import numpy as np

from .engine import Engine
from .scoring import Weights, ideal_score, score
from .task import Kind, Placement, Task


def _spread_cells(columns: int, rows: int, count: int) -> list[tuple[int, int]]:
    """``count`` cells of a ``rows`` x ``columns`` grid chosen one by one so that as few as possible are
    walled in (all four neighbours taken, the grid edge counting as a wall): what the ``enclosed`` term punishes."""

    def walled_in(cells: set[tuple[int, int]]) -> int:
        def blocked(r: int, c: int) -> bool:
            return not (0 <= r < rows and 0 <= c < columns) or (r, c) in cells

        return sum(all(blocked(r + dr, c + dc) for dr, dc in ((-1, 0), (1, 0), (0, -1), (0, 1))) for r, c in cells)

    chosen: list[tuple[int, int]] = []
    free = [(r, c) for r in range(rows) for c in range(columns)]
    for _ in range(count):
        best = min(free, key=lambda cell: (walled_in({*chosen, cell}), cell))
        chosen.append(best)
        free.remove(best)
    return chosen


def packed_layout(kinds: list[Kind], counts: list[int], width: int, height: int) -> list[Placement]:
    """``counts[i]`` machines of ``kinds[i]`` in equal-size cells, spread so that none is walled in while it can be
    (an inserter needs a free tile beside a machine, see the ``enclosed`` term)."""
    cell_w, cell_h = kinds[0].width, kinds[0].height
    if any((k.width, k.height) != (cell_w, cell_h) for k in kinds):
        raise ValueError("kinds must share a footprint")
    columns, rows = width // cell_w, height // cell_h
    wanted = [kind for kind, n in zip(kinds, counts) for _ in range(n)]
    if len(wanted) > columns * rows:
        raise ValueError("more machines than the grid holds")
    cells = _spread_cells(columns, rows, len(wanted))
    return [Placement(kind.machine, kind.recipe, c * cell_w, r * cell_h) for kind, (r, c) in zip(wanted, cells)]


def reference_score(engine: Engine, task: Task, weights: Weights | None = None) -> float:
    """The score to compare an agent with: the brute-force expert's when it applies, otherwise the
    `ideal_score` (every term at its best; integer machine counts usually stay below it)."""
    try:
        return score(expert_layout(engine, task, weights)[1], weights)
    except ValueError:
        return ideal_score(weights)


def expert_layout(
    engine: Engine, task: Task, weights: Weights | None = None
) -> tuple[list[Placement], dict]:
    """Best layout of two machine kinds (e.g. cable + circuit) by trying every machine count.

    Only for tasks with two kinds of equal footprint (else ``ValueError``). Returns the layout and its report.
    """
    kinds = engine.kinds(task)
    if len(kinds) != 2 or len({(k.width, k.height) for k in kinds}) != 1:
        raise ValueError("the expert handles exactly two kinds of equal footprint")
    capacity = (task.width // kinds[0].width) * (task.height // kinds[0].height)
    best: tuple[float, int, list[Placement], dict] | None = None
    for first in range(capacity + 1):
        for second in range(capacity - first + 1):
            layout = packed_layout(kinds, [first, second], task.width, task.height)
            report = engine.evaluate(task, layout)
            key = (score(report, weights), -len(layout))
            if best is None or key > best[:2]:
                best = (*key, layout, report)
    assert best is not None
    return best[2], best[3]


def genetic_search(
    engine: Engine,
    task: Task,
    population: int = 60,
    generations: int = 60,
    seed: int = 0,
    weights: Weights | None = None,
    on_generation: Callable[[int, list[Placement], dict], None] | None = None,
) -> tuple[list[Placement], dict]:
    """A small genetic algorithm over layouts.

    A genome is a list of ``(kind, y, x)`` genes. Decoding places them in order and skips the ones
    that do not fit (the engine's legal-action mask repairs overlaps), so every genome is valid.
    Fitness is ``scoring.score``. ``on_generation(generation, layout, report)`` is called with the best
    layout so far after every generation (the live viewer uses it). Meant as a template for your own search, not as the best one.
    """
    rng = np.random.default_rng(seed)
    kinds = engine.kinds(task)
    length = (task.width * task.height) // min(k.width * k.height for k in kinds)

    def random_gene() -> tuple[int, int, int]:
        return int(rng.integers(len(kinds))), int(rng.integers(task.height)), int(rng.integers(task.width))

    def decode(genome: list[tuple[int, int, int]]) -> list[Placement]:
        layout: list[Placement] = []
        for kind_index, y, x in genome:
            if engine.legal_actions(task, layout)[kind_index, y, x]:
                kind = kinds[kind_index]
                layout.append(Placement(kind.machine, kind.recipe, x, y))
        return layout

    def evaluate(genome):
        layout = decode(genome)
        report = engine.evaluate(task, layout)
        return score(report, weights), layout, report

    def mutate(genome):
        genome = list(genome)
        for _ in range(int(rng.integers(1, 4))):
            roll = rng.random()
            if roll < 0.4 and genome:
                genome[int(rng.integers(len(genome)))] = random_gene()
            elif roll < 0.7 and len(genome) < length:
                genome.insert(int(rng.integers(len(genome) + 1)), random_gene())
            elif genome:
                genome.pop(int(rng.integers(len(genome))))
        return genome

    pool = [[random_gene() for _ in range(int(rng.integers(1, length + 1)))] for _ in range(population)]
    best = max((evaluate(g) + (g,) for g in pool), key=lambda e: e[0])
    for generation in range(generations):
        scored = sorted(((evaluate(g)[0], g) for g in pool), key=lambda e: e[0], reverse=True)
        elite = [g for _, g in scored[: population // 4]]
        children = []
        while len(elite) + len(children) < population:
            a, b = (elite[int(rng.integers(len(elite)))] for _ in range(2))
            cut_a, cut_b = int(rng.integers(len(a) + 1)), int(rng.integers(len(b) + 1))
            children.append(mutate(a[:cut_a] + b[cut_b:]))
        pool = elite + children
        top = evaluate(elite[0])
        if top[0] > best[0]:
            best = top + (elite[0],)
        if on_generation:
            on_generation(generation, best[1], best[2])
    return best[1], best[2]
