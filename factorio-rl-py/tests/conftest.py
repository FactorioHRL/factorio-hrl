from pathlib import Path

import pytest

from factorio_rl import Engine

FIXTURES = Path(__file__).parent / "fixtures"


@pytest.fixture(scope="session")
def engine() -> Engine:
    """Engine on a small excerpt of the vanilla dump kept in ``tests/fixtures``."""
    return Engine(FIXTURES / "data-excerpt.json")


#: reward weights that switch the layout-shape terms off, leaving output and waste
NO_SHAPE = {"adjacency": 0.0, "line": 0.0, "stray": 0.0, "enclosed": 0.0, "cut_off": 0.0, "link": 0.0}
