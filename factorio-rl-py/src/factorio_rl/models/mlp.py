"""The plain network: the grid and the task vector are flattened and go through a fully connected
net. Simple and fast on small grids; it has no notion of which tiles are neighbours."""

from dataclasses import dataclass

from . import Architecture


@dataclass(frozen=True)
class Options:
    #: sizes of the hidden layers of the policy and of the value network
    net_arch: tuple[int, ...] = (64, 64)


ARCHITECTURE = Architecture(
    policy="MultiInputPolicy",
    options=Options,
    policy_kwargs=lambda o: {"net_arch": list(o.net_arch)},
)
