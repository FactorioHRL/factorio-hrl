use serde::{Deserialize, Serialize};

/// A reason a layout is not acceptable. `index` points into the placement list (or, for an
/// imported blueprint, the entity list).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Violation {
    OutOfBounds {
        index: usize,
    },
    Overlap {
        a: usize,
        b: usize,
    },
    MachineNotAllowed {
        index: usize,
        name: String,
    },
    RecipeNotAllowed {
        index: usize,
        name: String,
    },
    CannotCraft {
        index: usize,
        machine: String,
        recipe: String,
    },
    UnknownPrototype {
        index: usize,
        name: String,
    },
    /// Imported blueprint entity that is not a machine with a recipe on a whole tile.
    UnsupportedEntity {
        index: usize,
        name: String,
    },
}

/// All machines of one kind and recipe in a layout, with their steady-state load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupReport {
    pub machine: String,
    pub recipe: String,
    pub count: usize,
    pub crafts_per_second: f64,
    /// Share of the group's capacity in use, `0..=1`.
    pub utilisation: f64,
}

/// Net balance of one item per second in the steady state: negative is taken from the supply,
/// positive is left over (the target, or a surplus of something else).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowReport {
    pub item: String,
    pub per_second: f64,
}

/// One placed machine with its occupied tiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacedView {
    pub index: usize,
    pub machine: String,
    pub recipe: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl PlacedView {
    /// Tiles between two footprints, summed over both axes (0 when they touch or overlap).
    pub(crate) fn gap(&self, other: &PlacedView) -> i32 {
        let along = |a0: i32, a_len: i32, b0: i32, b_len: i32| {
            (a0.max(b0) - (a0 + a_len).min(b0 + b_len)).max(0)
        };
        along(self.x, self.width, other.x, other.width)
            + along(self.y, self.height, other.y, other.height)
    }
}

/// The distance in tiles from which two machines count as "far apart": half the larger grid
/// side. The distance terms cap at it and divide by it.
pub(crate) fn far_tiles(width: i32, height: i32) -> f64 {
    f64::from(width.max(height) / 2).max(1.0)
}

/// Everything known about a layout for a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// False when `violations` is not empty. An invalid layout produces nothing.
    pub valid: bool,
    pub violations: Vec<Violation>,
    pub placed: Vec<PlacedView>,
    pub machines: usize,
    pub tiles_used: i64,
    pub tiles_total: i64,
    /// Net production of the task's target item per second.
    pub output_per_second: f64,
    /// Most the grid could make of the target (ignores shape, assumes the best machine split).
    pub ceiling_per_second: f64,
    /// `output / ceiling`, `0..=1`.
    pub output_ratio: f64,
    /// Machines' worth of capacity that sits unused (sum of `count * (1 - utilisation)`).
    pub idle_machines: f64,
    /// `idle_machines` against the grid's machine budget (how many of the smallest allowed
    /// machine fit), `0..=1`. Not against the machines built so far: a half-built chain would
    /// then rate as pure waste.
    pub waste_ratio: f64,
    /// Layout shape, each `0..=1` (see `shape.rs` for the exact definitions): how fully
    /// machines touch twins (same machine and recipe) ...
    pub adjacency_ratio: f64,
    /// ... how many stand in a straight line of at least three twins ...
    pub line_ratio: f64,
    /// ... and how far single twins sit from the line their kind already has.
    pub stray_ratio: f64,
    /// Reachability (see `access.rs`), each over the machines that fit on the grid: machines
    /// with no free tile at any side (no inserter could reach them) ...
    pub enclosed_ratio: f64,
    /// ... and machines whose free tiles are all in pockets sealed off from the grid edge.
    pub cut_off_ratio: f64,
    /// Distance from each machine to the nearest machine making each of its ingredients (see
    /// `links.rs`), over the machines that fit on the grid, `0..=1`: a proxy for belt length.
    pub link_ratio: f64,
    pub groups: Vec<GroupReport>,
    /// Net balance per item: what the layout draws from the supply and what it leaves over.
    pub flows: Vec<FlowReport>,
}
