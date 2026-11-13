"""How a report is turned into one number: a sum of weighted *reward terms*.

Every term reads the engine's report and returns a ratio in ``0..1``. Its weight says how much it
counts; a negative weight makes it a penalty. The same ``score`` is used by the RL environment (the
reward of a step is the change of the score, so the episode return equals the final score), the
baselines, the genetic search and the viewer.

To add a term, write a function and register it (see ``REWARDS.md``)::

    @reward_term("my_term", weight=-0.1)
    def my_term(report: dict) -> float:
        \"\"\"One line saying what it measures.\"\"\"
        return ...  # a ratio in 0..1 from the report

Weights are overridden per run in the training config (``[reward]``). Geometry over many machines
belongs in Rust (``fsim-env``) so it stays cheap: expose a ratio in the report and read it here.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass

Report = dict
Weights = Mapping[str, float]


@dataclass(frozen=True)
class Term:
    name: str
    #: default weight; negative for a penalty
    weight: float
    value: Callable[[Report], float]

    @property
    def doc(self) -> str:
        return (self.value.__doc__ or "").strip()


#: every registered term, in registration order
TERMS: dict[str, Term] = {}


def reward_term(name: str, weight: float) -> Callable[[Callable[[Report], float]], Callable[[Report], float]]:
    """Registers ``fn(report) -> ratio in 0..1`` as the reward term ``name`` with a default ``weight``."""

    def register(fn: Callable[[Report], float]) -> Callable[[Report], float]:
        if name in TERMS:
            raise ValueError(f"reward term {name!r} is registered twice")
        TERMS[name] = Term(name, weight, fn)
        return fn

    return register


@reward_term("output", weight=1.0)
def output(report: Report) -> float:
    """Output as a share of the best possible (or of the required output)."""
    return report["output_ratio"]


@reward_term("waste", weight=-2.0)
def waste(report: Report) -> float:
    """Share of machine capacity that sits idle: building more than the supply or the target needs."""
    return report["waste_ratio"]


@reward_term("adjacency", weight=0.2)
def adjacency(report: Report) -> float:
    """Machines doing the same thing touching along a whole side (more such neighbours, higher)."""
    return report["adjacency_ratio"]


@reward_term("line", weight=0.3)
def line(report: Report) -> float:
    """Machines standing in a straight line of at least three that do the same thing (longer, higher)."""
    return report["line_ratio"]


@reward_term("stray", weight=-0.1)
def stray(report: Report) -> float:
    """How scattered machines doing the same thing are; a line counts as one machine, no line needed."""
    return report["stray_ratio"]


@reward_term("enclosed", weight=-2.0)
def enclosed(report: Report) -> float:
    """Machines walled in on every side (no free tile for an inserter), per machine that fits on the grid."""
    return report["enclosed_ratio"]


@reward_term("cut_off", weight=-2.0)
def cut_off(report: Report) -> float:
    """Machines whose free side tiles are all sealed off from the grid edge, per machine that fits on the grid."""
    return report["cut_off_ratio"]


@reward_term("link", weight=-0.5)
def link(report: Report) -> float:
    """Distance from each machine to the machines making its ingredients (a proxy for belt length), per machine that fits on the grid."""
    return report["link_ratio"]


def resolve_weights(overrides: Weights | None = None) -> dict[str, float]:
    """The default weight of every term with ``overrides`` applied; unknown names are an error."""
    overrides = dict(overrides or {})
    unknown = set(overrides) - set(TERMS)
    if unknown:
        raise ValueError(f"unknown reward terms {sorted(unknown)} (known: {sorted(TERMS)})")
    return {name: overrides.get(name, term.weight) for name, term in TERMS.items()}


def breakdown(report: Report, weights: Weights | None = None) -> dict[str, float]:
    """The weighted value of every term (what each one adds to or takes from the score)."""
    return {name: (weights or {}).get(name, term.weight) * term.value(report) for name, term in TERMS.items()}


def score(report: Report, weights: Weights | None = None) -> float:
    """Sum of the weighted terms: 1.0 means the target is met with no idle machine."""
    return sum(breakdown(report, weights).values())


def ideal_score(weights: Weights | None = None) -> float:
    """Best possible score: every term at its best (rewards at 1, penalties at 0)."""
    return sum(max((weights or {}).get(name, term.weight), 0.0) for name, term in TERMS.items())
