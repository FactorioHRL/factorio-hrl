"""Tasks the tests share, built in code: nothing here depends on a file outside the module and ``tests/``."""

from factorio_rl.config import Supply, TaskSpec
from factorio_rl.task import Task

LDS = "low-density-structure"
LDS_RECIPES = (LDS, "plastic-bar")
LDS_MACHINES = ("assembling-machine-2", "chemical-plant")

PACK = "production-science-pack"
PRODUCTION_RECIPES = (
    PACK, "rail", "iron-stick", "electric-furnace", "productivity-module", "advanced-circuit",
    "electronic-circuit", "copper-cable", "plastic-bar", "steel-plate", "stone-brick",
)
PRODUCTION_MACHINES = ("assembling-machine-2", "chemical-plant", "electric-furnace")
PRODUCTION_RAW = ("iron-plate", "copper-plate", "coal", "stone", "petroleum-gas")


def lds_task(width: int = 15, height: int = 15, copper: float = 15, steel: float = 15, coal: float = 15) -> Task:
    """Low density structures: copper, steel and coal limited, petroleum gas unlimited."""
    supply = {"copper-plate": copper, "steel-plate": steel, "coal": coal, "petroleum-gas": None}
    return Task(width, height, supply, LDS_RECIPES, LDS_MACHINES, LDS)


def lds_spec(*, grid_limited: bool = False) -> TaskSpec:
    """The family of low density structure tasks (12x12, or all inputs unlimited when ``grid_limited``)."""
    raw = ("copper-plate", "steel-plate", "coal", "petroleum-gas")
    supply = {item: Supply(unlimited=True) if grid_limited or item == "petroleum-gas" else Supply(low=1, high=15) for item in raw}
    return TaskSpec(
        name="lds", mode="grid_limited" if grid_limited else "supply_limited", target=LDS,
        recipes=LDS_RECIPES, machines=LDS_MACHINES, width_range=(12, 12), height_range=(12, 12), supply=supply,
    )


def production_task(width: int, height: int, supply: dict[str, float | None]) -> Task:
    return Task(width, height, supply, PRODUCTION_RECIPES, PRODUCTION_MACHINES, PACK)


def unlimited_production_task(width: int, height: int) -> Task:
    return production_task(width, height, {item: None for item in PRODUCTION_RAW})


def small_target_spec() -> TaskSpec:
    """Green circuits on 12x12 with unlimited inputs and 4 circuits per second asked for."""
    free = Supply(unlimited=True)
    return TaskSpec(
        name="small", mode="target_output", target="electronic-circuit",
        recipes=("copper-cable", "electronic-circuit"), machines=("assembling-machine-1",),
        width_range=(12, 12), height_range=(12, 12),
        supply={"iron-plate": free, "copper-plate": free}, required_output=Supply(fixed=4.0),
    )
