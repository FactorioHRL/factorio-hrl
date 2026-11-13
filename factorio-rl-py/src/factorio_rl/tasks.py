"""Handy task constructors for tests and notebooks (runs are configured with TOML, see ``config``)."""

from __future__ import annotations

from .task import Task

IRON = "iron-plate"
COPPER = "copper-plate"
CIRCUIT = "electronic-circuit"


def green_circuit_task(
    width: int,
    height: int,
    iron: float | None,
    copper: float | None,
    machines: tuple[str, ...] = ("assembling-machine-1",),
) -> Task:
    """Plates in, green circuits out, built from assemblers only (``None`` = unlimited supply)."""
    return Task(
        width=width,
        height=height,
        supply={IRON: iron if iron is None else float(iron), COPPER: copper if copper is None else float(copper)},
        recipes=("copper-cable", CIRCUIT),
        machines=machines,
        target=CIRCUIT,
    )
