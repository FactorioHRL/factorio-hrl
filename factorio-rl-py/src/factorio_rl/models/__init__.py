"""Network architectures for the neural algorithms, one module per architecture.

Every architecture is driven by the same training TOML; it is picked by name in ``[model]``::

    [model]
    architecture = "cnn"
    [model.options]          # only what this architecture takes; defaults when absent
    channels = [32, 64, 64, 64]

**The interface.** A module ``models/<name>.py`` defines a module-level ``ARCHITECTURE``, an
``Architecture``: the policy MaskablePPO should use, a dataclass of the architecture's options
(with defaults), and a function turning those options into the policy's keyword arguments. The
name of the module is the name used in the TOML; dropping a new file into this folder is all it
takes to add one (see ``mlp.py`` for the smallest example, ``cnn.py`` for a custom policy). The
training loop, the evaluation, MLflow and the live viewer never look inside an architecture.
"""

from __future__ import annotations

import importlib
import pkgutil
from collections.abc import Callable
from dataclasses import dataclass, fields
from typing import Any


@dataclass(frozen=True)
class Architecture:
    """What the training loop needs to build a network."""

    #: a policy class, or the name of one of the policies of stable-baselines3
    policy: Any
    #: dataclass of this architecture's options; every field has a default
    options: type
    #: the options, validated, as keyword arguments of the policy
    policy_kwargs: Callable[[Any], dict[str, Any]]


#: the names of the architectures in this folder (the modules whose name does not start with ``_``)
ARCHITECTURES: tuple[str, ...] = tuple(sorted(m.name for m in pkgutil.iter_modules(__path__) if not m.name.startswith("_")))


def load_architecture(name: str) -> Architecture:
    """The architecture ``name``. Imported on demand, so listing names never imports torch."""
    if name not in ARCHITECTURES:
        raise ValueError(f"unknown architecture {name!r} (known: {list(ARCHITECTURES)})")
    return importlib.import_module(f"{__name__}.{name}").ARCHITECTURE


def policy_for(name: str, options: dict[str, Any]) -> tuple[Any, dict[str, Any]]:
    """``(policy, policy_kwargs)`` of the architecture ``name`` with the TOML ``options`` applied."""
    architecture = load_architecture(name)
    allowed = {f.name for f in fields(architecture.options)}
    unknown = set(options) - allowed
    if unknown:
        raise ValueError(f"[model.options] of {name!r}: unknown keys {sorted(unknown)} (allowed: {sorted(allowed)})")
    return architecture.policy, architecture.policy_kwargs(architecture.options(**options))
