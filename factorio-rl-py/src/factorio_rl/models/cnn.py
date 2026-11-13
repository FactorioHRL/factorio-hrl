"""A fully convolutional network: the logit of every (kind, tile) is computed from the tile's
neighbourhood, so "put it next to the machine over there" is learned once for the whole grid
instead of once per position. This is the standard design for placement tasks.

    observation grid (kinds + 1, H, W)  +  task vector tiled over the grid
      -> a few 3x3 convolutions (dilated, so the view grows quickly)
      -> 1x1 convolution: one logit per kind and tile          (the action logits)
      -> pooled features: the logit of "stop" and the value of the state

The action layout is the environment's: ``(kind * max_height + y) * max_width + x`` and last "stop".
"""

from dataclasses import dataclass
from functools import partial

import numpy as np
import torch
from sb3_contrib.common.maskable.policies import MaskableMultiInputActorCriticPolicy
from torch import nn

from . import Architecture


@dataclass(frozen=True)
class Options:
    #: output channels of the 3x3 convolutions
    channels: tuple[int, ...] = (32, 64, 64, 64)
    #: dilation of each convolution (same length as ``channels``); with 1, 2, 4, 1 a tile sees 17x17 tiles
    dilations: tuple[int, ...] = (1, 2, 4, 1)

    def __post_init__(self) -> None:
        if len(self.channels) != len(self.dilations) or not self.channels:
            raise ValueError("channels and dilations must be non-empty and of the same length")


class ConvBody(nn.Module):
    """Features (the flattened grid, then the task vector) -> action logits and state features."""

    def __init__(self, planes: int, height: int, width: int, task_dim: int, channels: tuple[int, ...], dilations: tuple[int, ...]) -> None:
        super().__init__()
        self.planes, self.height, self.width = planes, height, width
        layers: list[nn.Module] = []
        previous = planes + task_dim
        for out, dilation in zip(channels, dilations):
            layers += [nn.Conv2d(previous, out, 3, padding=dilation, dilation=dilation), nn.ReLU()]
            previous = out
        self.body = nn.Sequential(*layers)
        kinds = planes - 1
        self.logits = nn.Conv2d(previous, kinds, 1)
        self.stop = nn.Linear(previous, 1)
        # the attributes SB3 reads from a policy's mlp_extractor
        self.latent_dim_pi = kinds * height * width + 1
        self.latent_dim_vf = 2 * previous

    def _maps(self, features: torch.Tensor) -> torch.Tensor:
        cells = self.planes * self.height * self.width
        grid = features[:, :cells].reshape(-1, self.planes, self.height, self.width)
        task = features[:, cells:][:, :, None, None].expand(-1, -1, self.height, self.width)
        return self.body(torch.cat([grid, task], dim=1))

    def forward(self, features: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
        maps = self._maps(features)
        pooled = maps.mean(dim=(2, 3))
        logits = torch.cat([self.logits(maps).flatten(1), self.stop(pooled)], dim=1)
        return logits, torch.cat([pooled, maps.amax(dim=(2, 3))], dim=1)

    def forward_actor(self, features: torch.Tensor) -> torch.Tensor:
        return self.forward(features)[0]

    def forward_critic(self, features: torch.Tensor) -> torch.Tensor:
        return self.forward(features)[1]


class ConvPolicy(MaskableMultiInputActorCriticPolicy):
    """MaskablePPO's policy with ``ConvBody`` in place of the fully connected extractor."""

    def __init__(self, *args, channels: tuple[int, ...] = Options.channels, dilations: tuple[int, ...] = Options.dilations, **kwargs) -> None:
        # needed by ``_build``, which the parent constructor calls
        self.channels, self.dilations = tuple(channels), tuple(dilations)
        super().__init__(*args, **kwargs)

    def _build_mlp_extractor(self) -> None:
        planes, height, width = self.observation_space["grid"].shape
        task_dim = self.observation_space["task"].shape[0]
        assert self.features_dim == planes * height * width + task_dim, "expects the flattened grid followed by the task vector"
        self.mlp_extractor = ConvBody(planes, height, width, task_dim, self.channels, self.dilations)

    def _build(self, lr_schedule) -> None:
        # SB3's own ``_build`` would add a linear layer from the logits to the logits (100M weights);
        # the logits are final here, so the layers are put together by hand
        self._build_mlp_extractor()
        self.action_net = nn.Identity()
        self.value_net = nn.Linear(self.mlp_extractor.latent_dim_vf, 1)
        body, head = self.mlp_extractor, self.mlp_extractor.logits
        body.apply(partial(self.init_weights, gain=np.sqrt(2)))
        head.apply(partial(self.init_weights, gain=0.01))  # a near-uniform policy to start from
        self.value_net.apply(partial(self.init_weights, gain=1))
        self.optimizer = self.optimizer_class(self.parameters(), lr=lr_schedule(1), **self.optimizer_kwargs)


ARCHITECTURE = Architecture(
    policy=ConvPolicy,
    options=Options,
    policy_kwargs=lambda o: {"channels": o.channels, "dilations": o.dilations},
)
