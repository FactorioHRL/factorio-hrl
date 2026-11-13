//! Naive input/output balance of a design: every machine with a recipe consumes ingredients and
//! produces results at a rate of `amount / (recipe_time / machine_speed)`, and the sum over
//! all machines gives the net balance. Ignores modules, beacons, quality,
//! belt throughput and whether the machines are connected at all.

use std::collections::{BTreeMap, HashMap};

use fsim_blueprint::model::Design;
use fsim_data::{RawData, Recipe};

mod steady;

pub use steady::{ceiling, steady_state, Group, SteadyState, Supply};

/// Errors of the calculator.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Data(#[from] fsim_data::Error),
    #[error("linear program failed: {0}")]
    Solver(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Net balance of one item or fluid per second. Negative is an input (consumed more
/// than produced), positive is an output.
#[derive(Debug, Clone, PartialEq)]
pub struct Flow {
    pub name: String,
    pub per_second: f64,
}

impl Flow {
    pub fn is_input(&self) -> bool {
        self.per_second < 0.0
    }
}

/// Items gained (positive) or consumed (negative) by one craft of `recipe`; results use the
/// expected amount (mean of `amount_min/max` times `probability`).
pub(crate) fn net_per_craft(recipe: &Recipe) -> Vec<(String, f64)> {
    let consumed = recipe
        .ingredients
        .iter()
        .map(|i| (i.name.clone(), -i.amount));
    let produced = recipe.results.iter().map(|p| {
        let average = (p.amount_min + p.amount_max) / 2.0 * p.probability;
        (p.name.clone(), average)
    });
    consumed.chain(produced).collect()
}

/// Net balance of all machines with a recipe. Inputs first (largest demand first),
/// then outputs (largest production first). Items with zero balance are skipped.
pub fn net_flows(design: &Design, data: &RawData) -> Result<Vec<Flow>> {
    let mut speeds: HashMap<String, Option<f64>> = HashMap::new();
    let mut net: BTreeMap<String, f64> = BTreeMap::new();
    for entity in &design.entities {
        let Some(recipe) = &entity.recipe else {
            continue;
        };
        let speed = match speeds.get(&*entity.name) {
            Some(speed) => *speed,
            None => {
                let speed = data.machine(&entity.name)?.map(|m| m.crafting_speed);
                speeds.insert(entity.name.to_string(), speed);
                speed
            }
        };
        let (Some(speed), Some(recipe)) = (speed.filter(|s| *s > 0.0), data.recipe(&recipe.name)?)
        else {
            continue;
        };
        let crafts_per_second = speed / recipe.energy_required;
        for (name, amount) in net_per_craft(&recipe) {
            *net.entry(name).or_default() += amount * crafts_per_second;
        }
    }
    Ok(into_flows(net))
}

/// Turns per-item net rates into sorted flows: inputs first (largest demand first), then
/// outputs (largest production first). Zero balances are dropped.
pub(crate) fn into_flows(net: BTreeMap<String, f64>) -> Vec<Flow> {
    let mut flows: Vec<Flow> = net
        .into_iter()
        .filter(|(_, rate)| rate.abs() > 1e-9)
        .map(|(name, per_second)| Flow { name, per_second })
        .collect();
    flows.sort_by(|a, b| {
        b.is_input()
            .cmp(&a.is_input())
            .then_with(|| b.per_second.abs().total_cmp(&a.per_second.abs()))
    });
    flows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> RawData {
        RawData::from_slice(
            br#"{
            "assembling-machine": {
                "slow": {"name": "slow", "crafting_speed": 0.5, "crafting_categories": ["crafting"]},
                "fast": {"name": "fast", "crafting_speed": 2.0, "crafting_categories": ["crafting"]}
            },
            "recipe": {
                "gear": {"name": "gear", "energy_required": 0.5,
                    "ingredients": [{"name": "plate", "amount": 2}],
                    "results": [{"name": "gear", "amount": 1}]},
                "bot": {"name": "bot", "energy_required": 2,
                    "ingredients": [{"name": "gear", "amount": 1}, {"name": "plate", "amount": 1}],
                    "results": [{"name": "bot", "amount": 1}]},
                "luck": {"name": "luck", "energy_required": 1,
                    "ingredients": [],
                    "results": [{"name": "ore", "amount_min": 1, "amount_max": 3, "probability": 0.5}]}
            }
        }"#,
        )
        .unwrap()
    }

    fn design(machines: &[(&str, &str)]) -> Design {
        use fsim_blueprint::model::{CenterPos, Direction, Entity, Quality, Recipe};
        Design {
            entities: machines
                .iter()
                .enumerate()
                .map(|(i, (name, recipe))| Entity {
                    name: (*name).into(),
                    pos: CenterPos::new(i as i32 * 4, 0),
                    direction: Direction::NORTH,
                    mirror: false,
                    quality: Quality::normal(),
                    recipe: Some(Recipe {
                        name: (*recipe).into(),
                        quality: Quality::normal(),
                    }),
                    modules: Vec::new(),
                    io: None,
                })
                .collect(),
        }
    }

    fn rate(flows: &[Flow], name: &str) -> Option<f64> {
        flows.iter().find(|f| f.name == name).map(|f| f.per_second)
    }

    #[test]
    fn rates_scale_with_crafting_speed() {
        // speed 0.5 / time 0.5 s = 1 craft/s: -2 plate, +1 gear
        let flows = net_flows(&design(&[("slow", "gear")]), &data()).unwrap();
        assert_eq!(rate(&flows, "plate"), Some(-2.0));
        assert_eq!(rate(&flows, "gear"), Some(1.0));
        let flows = net_flows(&design(&[("fast", "gear")]), &data()).unwrap();
        assert_eq!(rate(&flows, "gear"), Some(4.0));
    }

    #[test]
    fn intermediate_products_cancel_between_machines() {
        // fast gear: +4 gear/s, -8 plate/s. slow bot: 0.25 craft/s -> -0.25 gear, -0.25 plate, +0.25 bot
        let flows = net_flows(&design(&[("fast", "gear"), ("slow", "bot")]), &data()).unwrap();
        assert_eq!(rate(&flows, "gear"), Some(3.75));
        assert_eq!(rate(&flows, "plate"), Some(-8.25));
        assert_eq!(rate(&flows, "bot"), Some(0.25));
    }

    #[test]
    fn exactly_balanced_items_disappear() {
        // slow gear: +1 gear/s; fast bot needs 1 craft/s -> -1 gear/s
        let flows = net_flows(&design(&[("slow", "gear"), ("fast", "bot")]), &data()).unwrap();
        assert_eq!(rate(&flows, "gear"), None);
    }

    #[test]
    fn probability_and_ranges_use_the_expectation() {
        let flows = net_flows(&design(&[("slow", "luck")]), &data()).unwrap();
        assert_eq!(rate(&flows, "ore"), Some(0.5 * 2.0 * 0.5));
    }

    #[test]
    fn inputs_come_first_then_outputs_largest_first() {
        let flows = net_flows(&design(&[("fast", "gear"), ("slow", "bot")]), &data()).unwrap();
        let order: Vec<_> = flows.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(order, ["plate", "gear", "bot"]);
    }

    #[test]
    fn machines_without_recipe_or_data_are_ignored() {
        let mut d = design(&[("slow", "gear"), ("ghost", "gear"), ("slow", "nope")]);
        d.entities[1].recipe = None;
        let flows = net_flows(&d, &data()).unwrap();
        assert_eq!(rate(&flows, "plate"), Some(-2.0));
    }
}
