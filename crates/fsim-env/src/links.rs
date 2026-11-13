//! Which recipes feed which, and how far apart the machines of a chain stand. A belt or an
//! inserter chain has to carry an ingredient from the machine that makes it to the machine that
//! uses it, so the distance between the two is a proxy for the logistics a real blueprint needs.
//! (Raw inputs, which no recipe of the task makes, come from outside the grid and are left out.)

use std::collections::BTreeMap;

use fsim_data::RawData;

use crate::error::Result;
use crate::report::PlacedView;

/// For every recipe of a task, and for each of its ingredients that another recipe of the task
/// makes, the recipes that make it.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Chain {
    suppliers: BTreeMap<String, Vec<Vec<String>>>,
}

impl Chain {
    pub(crate) fn new(data: &RawData, recipes: &[String]) -> Result<Self> {
        let mut parsed = Vec::new();
        let mut makers: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for name in recipes {
            if let Some(recipe) = data.recipe(name)? {
                for product in &recipe.results {
                    makers
                        .entry(product.name.clone())
                        .or_default()
                        .push(recipe.name.clone());
                }
                parsed.push(recipe);
            }
        }
        let suppliers = parsed
            .iter()
            .map(|recipe| {
                let feeding = recipe
                    .ingredients
                    .iter()
                    .filter_map(|ingredient| makers.get(&ingredient.name))
                    // a recipe that makes its own ingredient does not feed itself
                    .map(|names| {
                        names
                            .iter()
                            .filter(|n| **n != recipe.name)
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .filter(|names| !names.is_empty())
                    .collect();
                (recipe.name.clone(), feeding)
            })
            .collect();
        Ok(Self { suppliers })
    }
}

/// Sum over every machine and every ingredient that has a maker in the layout of the distance
/// (tiles between footprints, capped at `far` and divided by it) to the nearest maker, over the
/// `budget` machines that fit on the grid, `0..=1`. A maker that is not built yet adds nothing:
/// missing it is the output term's business, and the distance is only meaningful once there is
/// something to be near. Each link costs the same wherever in the episode it is made.
pub(crate) fn link_ratio(placed: &[PlacedView], chain: &Chain, far: f64, budget: usize) -> f64 {
    if budget == 0 {
        return 0.0;
    }
    let mut by_recipe: BTreeMap<&str, Vec<&PlacedView>> = BTreeMap::new();
    for machine in placed {
        by_recipe.entry(&machine.recipe).or_default().push(machine);
    }
    let mut total = 0.0;
    for machine in placed {
        for makers in chain.suppliers.get(&machine.recipe).into_iter().flatten() {
            let nearest = makers
                .iter()
                .filter_map(|name| by_recipe.get(name.as_str()))
                .flatten()
                .map(|maker| machine.gap(maker))
                .min();
            if let Some(gap) = nearest {
                total += (f64::from(gap) / far).min(1.0);
            }
        }
    }
    (total / budget as f64).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// gear -> pack, and pack needs a raw item (copper) as well
    fn chain() -> Chain {
        let data = RawData::from_slice(
            br#"{"recipe": {
                "gear": {"name": "gear", "ingredients": [{"name": "iron", "amount": 2}],
                    "results": [{"name": "gear", "amount": 1}]},
                "pack": {"name": "pack", "ingredients": [{"name": "copper", "amount": 1},
                    {"name": "gear", "amount": 1}], "results": [{"name": "pack", "amount": 1}]}
            }}"#,
        )
        .unwrap();
        Chain::new(&data, &["gear".into(), "pack".into()]).unwrap()
    }

    fn at(recipe: &str, x: i32, y: i32) -> PlacedView {
        PlacedView {
            index: 0,
            machine: "m".into(),
            recipe: recipe.into(),
            x,
            y,
            width: 3,
            height: 3,
        }
    }

    #[test]
    fn only_ingredients_another_recipe_makes_are_links() {
        let chain = chain();
        assert_eq!(chain.suppliers["pack"], [vec!["gear".to_owned()]]);
        assert!(chain.suppliers["gear"].is_empty(), "iron is a raw input");
    }

    #[test]
    fn a_consumer_next_to_its_maker_costs_nothing_and_a_far_one_costs_a_link() {
        let chain = chain();
        let touching = [at("gear", 0, 0), at("pack", 3, 0)];
        assert_eq!(link_ratio(&touching, &chain, 15.0, 100), 0.0);
        // 3 tiles apart on a grid whose "far" is 6: half a link; budget 10 machines
        let apart = [at("gear", 0, 0), at("pack", 6, 0)];
        assert!((link_ratio(&apart, &chain, 6.0, 10) - 0.5 / 10.0).abs() < 1e-12);
        let far = [at("gear", 0, 0), at("pack", 24, 0)];
        assert!(
            (link_ratio(&far, &chain, 6.0, 10) - 1.0 / 10.0).abs() < 1e-12,
            "capped at one"
        );
    }

    #[test]
    fn the_nearest_maker_counts_and_each_consumer_has_its_own_link() {
        let chain = chain();
        let layout = [
            at("gear", 0, 0),
            at("gear", 30, 0),
            at("pack", 3, 0),
            at("pack", 33, 0),
        ];
        assert_eq!(
            link_ratio(&layout, &chain, 15.0, 100),
            0.0,
            "each pack has a gear beside it"
        );
        let one_far = [at("gear", 0, 0), at("pack", 3, 0), at("pack", 60, 0)];
        assert!((link_ratio(&one_far, &chain, 15.0, 100) - 1.0 / 100.0).abs() < 1e-12);
    }

    #[test]
    fn nothing_is_charged_before_the_maker_exists() {
        assert_eq!(link_ratio(&[at("pack", 0, 0)], &chain(), 15.0, 100), 0.0);
        assert_eq!(link_ratio(&[], &chain(), 15.0, 100), 0.0);
        assert_eq!(
            link_ratio(&[at("gear", 0, 0), at("pack", 60, 0)], &chain(), 15.0, 0),
            0.0
        );
    }
}
