# Rewards and penalties

The score of a layout is a sum of weighted *reward terms* (`src/factorio_rl/scoring.py`). The
environment rewards a step with the **change of the score**, so the return of an episode equals
the final score and a term can never be farmed by building and removing things.

Every term is a ratio in `0..1` read from the engine's report. Its weight says how much it
counts; a negative weight is a penalty. 1.0 means the target is met with no idle machine; the
best possible score (`scoring.ideal_score`) is the sum of the positive weights.

## The terms

| term | default weight | measures |
|---|---|---|
| `output` | +1.0 | output as a share of the best possible, or of the required output (`target_output` tasks) |
| `waste` | -2.0 | idle machines (capacity that is not used) as a share of the machines that fit on the grid |
| `adjacency` | +0.2 | twins touching along a **whole** side |
| `line` | +0.3 | twins standing in a straight line of at least three |
| `stray` | -0.1 | twins scattered over the grid (a line counts as one machine; no line needed) |
| `enclosed` | -2.0 | machines walled in on every side: no free tile for an inserter |
| `cut_off` | -2.0 | machines whose free side tiles are all sealed off from the grid edge |
| `link` | -0.5 | distance from each machine to the machines that make its ingredients |

*Twins* are machines with the same machine name and recipe. The exact geometry is documented in
`crates/fsim-env/src/shape.rs` (it runs in Rust so it stays cheap: O(n log n) per step, with no
measurable slowdown of the environment).

- **adjacency**: twins touching along a **whole** side (same row or column, no offset) form a
  *cluster*. A cluster that is one straight run (all in one row or all in one column) of at least
  two machines earns `machines - 1` contacts; a cluster with a bend, a branch or a second row (a
  T, an L, a block, a snake) earns nothing. The term is the contacts over the machines of kinds
  built at least twice: a pair rates 0.5, a long row almost 1.
- **line**: twins joined by full contact form a *cluster*. A cluster is a line when it is one
  straight run, all in one row or all in one column, of at least three machines. A cluster with a
  bend, a branch or a second row (a T, an L, a domino, a block) is no line, and none of its machines
  is: growing a line into a maze loses the reward instead of earning it twice. Each machine of a
  kind built at least three times rates `min(length of its line, 6) / 6`; the term is the mean. A
  twin that touches a row only half-way is not in full contact and does not break the row.
- **stray**: how scattered a kind is. Its machines form *units*: a line is one unit (its machines
  count as a single machine, so building a line is never punished) and every machine outside a
  line is a unit of its own (a block or a T is just touching single machines). The units are joined by the
  shortest possible links (a minimum spanning tree; a link is the tiles between two footprints);
  each link counts its length capped at half the larger grid side and divided by it. A kind
  rates `sum of its links / (machines - 1)`:
  - one unit alone, or a compact cluster of single machines: 0;
  - two far lines of five: 1/9 (small);
  - a line of three and seven machines strewn about: 7/9;
  - ten machines all apart and far from each other: 1.
  Only the shortest links count, so a new line or cluster started away from the rest costs one
  link, not one per machine: starting a line is never a trap. The term is the mean over the
  machines of kinds built at least twice and needs no line to start working, so it pulls single
  machines together before the agent has learned to build lines.
- **enclosed**: a machine with no free tile along any of its sides cannot be reached by an
  inserter, so it is useless in a real blueprint. Tiles outside the grid count as walls (the grid
  is the whole area a layout may use), so a machine in a corner needs a free tile on one of its
  two open sides. One free tile is enough (`MIN_FREE_TILES` in `shape.rs`). The value is the number
  of enclosed machines over the machines that fit on the grid, so each one costs the same
  (-2 / 113 = about -0.018 on 32x32 with 3x3 machines) and an extra machine never dilutes it.
  Enclosed machines still count as working in the output and waste terms; this term is what makes
  them a mistake. Packing a block of machines tightly is therefore a large loss: leave
  gaps, like a real factory does for its inserters and belts.

- **cut_off**: the next step after `enclosed`. A machine can have a free tile beside it and still
  be unreachable: the tile may sit in a pocket of free space that other machines have sealed off
  from the grid edge, where inputs and outputs pass. Free tiles are connected through their four
  neighbours; the flood starts from the free tiles on the edge of the grid. A machine is *open*
  when at least one free tile beside it connects to the edge, *cut off* when it has free tiles
  but none of them does, and *enclosed* when it has none (`access.rs`). The two penalties are
  disjoint, so a machine costs once, and both are measured against the machines that fit on
  the grid, like `waste`.

- **link**: a belt has to carry an ingredient from the machine that makes it to the machine that
  uses it, so the distance between the two stands in for the logistics of a real blueprint (the
  same idea as wirelength in chip placement). For every machine and every ingredient that another
  recipe of the task makes, take the distance in tiles to the *nearest* machine making it, cap it
  at half the larger grid side and divide by it. The sum is divided by the machines that fit on the
  grid, so each link costs the same wherever in the episode it is made. Raw inputs (nobody in the
  task makes them) come from outside and are left out; a maker that is not built yet adds nothing,
  since missing it is the output term's business. Unlike `adjacency`, this relates *different*
  recipes: a gear assembler next to the pack assembler that uses its gears (`links.rs`).

## Geometry that gates the output (`require_access`)

The terms above punish a bad geometry; they do not stop a walled-in machine from producing. A task
can say so with `require_access = true` (top level of the task file): machines that are enclosed or
cut off then make nothing and count as idle (`waste`). The output is the real output of the
machines that can be reached, so a layout that only works on paper scores badly on `output` itself.

It is off by default because it makes the signal harsher: the output drops to zero whenever the
last machine of a chain is walled in, which is a sparser reward for a beginner. A sound use is the
second phase of a run, once the agent builds layouts that meet the output.

## Setting the weights

Per run, in the training config (unlisted terms keep their default):

```toml
[reward]
line = 0.1        # a bigger bonus for lines
stray = 0         # switch a term off
waste = -4.0      # a stronger penalty
```

Unknown names fail at start with the list of known terms. Weights apply to the environment, the
evaluation, the expert reference, the genetic search and the live viewer alike. The live viewer
shows what each term adds to the score, and MLflow gets `eval_term_<name>` (the mean weighted
contribution per evaluation), so you can see which part of the score is moving.

What the agent sees: besides the grid, the observation holds, per kind of machine, how many
stand and how busy they are, so it can tell which stage of a chain is the bottleneck.

A term has to be big enough to learn from. The shape terms are means over all machines, so one
more twin moves them very little: with the first weights (0.05) a new line of three on a
45-machine layout was worth +0.0035 on a score of 0.83, below the noise of PPO, and the agent never
built one. With 0.2 (adjacency) and 0.3 (line) it is worth about 0.02, the same as two idle
machines. Check a new term's weight the same way: build the layout with and without the feature
and compare `scoring.score`; a gain under 1% of the score will not be learned.

One more rule: a machine added only for its shape reward must cost more than it earns. A line of
five idle machines built to collect the line bonus earned about +0.007 per machine with `waste`
at -1 (idle machine: -0.009 on a 113-machine grid), so `waste` is -2 by default (-0.018).

The shape terms are still hints about tidy layouts, not the goal: the best they can add together
is 0.5 against 1.0 for the output. If the agent builds neat lines of useless machines, lower them
(or raise `waste`).

Two phases work well for a hard task: first train with `adjacency = line = stray = 0` until
`eval_term_output` reaches 1 and `eval_term_waste` is small, then switch the shape terms on and
continue from the saved model. `configs/training/ppo_large_phase1.toml` and `ppo_large_phase2.toml`
do exactly that (`[ppo] init_from` points phase 2 at the `best.zip` of phase 1)

## Adding a term

1. If the term needs geometry or anything over many machines, compute it in Rust and expose a
   ratio in the report (`crates/fsim-env/src/shape.rs`, `report.rs`, then `Engine::report`).
   Anything cheap can read the report directly in Python, including `report["placed"]` (every
   machine with its position and size).
2. Register a function in `scoring.py`:

   ```python
   @reward_term("tiles_used", weight=-0.1)
   def tiles_used(report: dict) -> float:
       """Share of the grid that is built over."""
       return report["tiles_used"] / report["tiles_total"]
   ```

   It must return a ratio in `0..1` (so `ideal_score` and the weights stay comparable).
3. That is all: the term is in the score, the breakdown in the viewer, the MLflow metrics and the
   `[reward]` config, and `tests/test_scoring.py` shows the pattern for a test.

Things to watch when designing a term:

- A term that *drops* when a machine is added can make the agent avoid building (this is why
  `stray` treats a line as one machine, so building a line never costs).
- Count a group once: `stray` merges the machines of a line into one unit, so the line reward and the distance penalty never pile up on the same machines.
- Prefer terms bounded by 1; an unbounded one can outgrow `output` and become the goal.
- Changing a term changes the meaning of the scores: runs before and after are not comparable.
