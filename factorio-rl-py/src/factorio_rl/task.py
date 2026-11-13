"""Plain data types shared with the Rust engine (they are sent across as JSON)."""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass, field

#: text drawings mark an occupied tile by the class of what stands there, not by its recipe or tier
#: (``M`` machine; ``I`` inserter and ``B`` belt are reserved for when those enter the model)
MACHINE_SYMBOL = "M"
EMPTY_SYMBOL = "."


def ascii_grid(width: int, height: int, placed: list[dict]) -> str:
    """The grid as text. ``placed`` is ``report["placed"]`` (machines with their footprints)."""
    rows = [[EMPTY_SYMBOL] * width for _ in range(height)]
    for view in placed:
        for y in range(view["y"], view["y"] + view["height"]):
            rows[y][view["x"] : view["x"] + view["width"]] = [MACHINE_SYMBOL] * view["width"]
    return "\n".join("".join(row) for row in rows)


@dataclass(frozen=True)
class Task:
    """What a layout must achieve: the grid, the incoming items and the allowed building blocks."""

    width: int
    height: int
    #: items arriving per second, e.g. ``{"iron-plate": 10, "copper-plate": 15}``
    supply: dict[str, float]
    #: recipes the layout may use
    recipes: tuple[str, ...]
    #: crafting machines the layout may use
    machines: tuple[str, ...]
    #: the item whose net production is maximised
    target: str
    #: output per second the layout is asked for (``None``: as much as the grid and supply allow)
    required_output: float | None = None
    #: machines nothing can reach (enclosed or cut off) make nothing and count as idle
    require_access: bool = False

    def to_json(self) -> str:
        return json.dumps(asdict(self))


@dataclass(frozen=True)
class Placement:
    """One machine; ``(x, y)`` is the top left tile of its footprint."""

    machine: str
    recipe: str
    x: int
    y: int


@dataclass(frozen=True)
class Kind:
    """A machine that can run a recipe, with its footprint: one choice of what to build."""

    machine: str
    recipe: str
    width: int
    height: int


@dataclass
class Layout:
    """A growing list of placements."""

    placements: list[Placement] = field(default_factory=list)

    def to_json(self) -> str:
        return json.dumps([asdict(p) for p in self.placements])


def layout_json(placements: list[Placement]) -> str:
    return Layout(list(placements)).to_json()
