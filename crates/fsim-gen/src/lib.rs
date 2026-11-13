//! Random blueprint generator.
//!
//! `generate` picks a random entity, position and direction and places it if it fits
//! (up to the requested entity count). The result is a `Design` from `fsim-blueprint`;
//! `to_blueprint_string` turns it into text to paste into the game. The generator knows footprints
//! from `fsim-data`, so a 3×3 assembler occupies 9 tiles and nothing overlaps.
//!
//! The old template generator (machine bands) lives in the `legacy` module.

mod canvas;
mod error;
pub mod legacy;
mod random;
mod rng;

use fsim_blueprint::model::Design;

pub use canvas::Canvas;
pub use error::{Error, Result};
pub use random::{candidate_names, generate};
pub use rng::Rng;

/// Default number of entities in a random blueprint.
pub const DEFAULT_ENTITIES: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    pub width: i32,
    pub height: i32,
    pub seed: u64,
    /// How many entities to place (ignored by `legacy`).
    pub entities: usize,
}

impl Params {
    pub fn square(size: i32, seed: u64) -> Self {
        Self {
            width: size,
            height: size,
            seed,
            entities: DEFAULT_ENTITIES,
        }
    }

    pub fn with_entities(self, entities: usize) -> Self {
        Self { entities, ..self }
    }
}

/// Blueprint as text to paste into the game.
pub fn to_blueprint_string(design: &Design) -> fsim_blueprint::Result<String> {
    design.to_blueprint_string()
}
