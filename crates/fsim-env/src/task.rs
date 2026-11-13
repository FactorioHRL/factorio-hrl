use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// What a layout is asked to do: the grid it must fit in, what arrives from outside, and the
/// building blocks it may use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    /// Grid size in tiles.
    pub width: i32,
    pub height: i32,
    /// Items arriving per second, e.g. `{"iron-plate": 10, "copper-plate": 15}`. A `null` rate
    /// means unlimited (the report then says how much the layout would draw).
    pub supply: BTreeMap<String, Option<f64>>,
    /// Recipes the layout may use.
    pub recipes: Vec<String>,
    /// Crafting machines the layout may use.
    pub machines: Vec<String>,
    /// The item whose net production is maximised.
    pub target: String,
    /// Output rate the layout is asked for. With it, the layout is rated against exactly this
    /// rate: producing more does not count and the machines it would need are idle.
    #[serde(default)]
    pub required_output: Option<f64>,
    /// Machines that nothing can reach (enclosed or cut off, see `access.rs`) do not work: they
    /// make nothing and count as idle. Off, they still work and only the reward terms object.
    #[serde(default)]
    pub require_access: bool,
}

impl Task {
    pub(crate) fn check(&self) -> Result<()> {
        if self.width < 1 || self.height < 1 {
            return Err(Error::InvalidTask(format!(
                "grid {}x{} must be at least 1x1",
                self.width, self.height
            )));
        }
        if self
            .supply
            .values()
            .flatten()
            .any(|rate| !rate.is_finite() || *rate < 0.0)
        {
            return Err(Error::InvalidTask(
                "supply rates must be finite and >= 0".into(),
            ));
        }
        if self
            .required_output
            .is_some_and(|rate| !rate.is_finite() || rate <= 0.0)
        {
            return Err(Error::InvalidTask(
                "required_output must be finite and > 0".into(),
            ));
        }
        Ok(())
    }
}

/// One machine on the grid. `(x, y)` is the top left tile of its footprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub machine: String,
    pub recipe: String,
    pub x: i32,
    pub y: i32,
}

/// A machine that can run a recipe, with its footprint: one choice of "what to build".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kind {
    pub machine: String,
    pub recipe: String,
    pub width: i32,
    pub height: i32,
}
