//! Naive input/output balance of a design: every machine with a recipe consumes ingredients and
//! produces results at a rate of `amount / (recipe_time / machine_speed)`, and the sum over
//! all machines gives the net balance. Ignores modules, beacons, quality,
//! belt throughput and whether the machines are connected at all.

use std::collections::{BTreeMap, HashMap};

use fsim_blueprint::model::Design;
use fsim_data::RawData;

use crate::error::Result;
use crate::text::{self, Weight};

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
        for ingredient in &recipe.ingredients {
            *net.entry(ingredient.name.clone()).or_default() -=
                ingredient.amount * crafts_per_second;
        }
        for product in &recipe.results {
            let average = (product.amount_min + product.amount_max) / 2.0 * product.probability;
            *net.entry(product.name.clone()).or_default() += average * crafts_per_second;
        }
    }
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
    Ok(flows)
}

/// Unsigned rate per second: `2.5/s`, `12.3/s`, `130/s` (fewer decimal places for
/// larger numbers).
pub(crate) fn format_rate(rate: f64) -> String {
    let magnitude = rate.abs();
    let digits = if magnitude >= 100.0 {
        0
    } else if magnitude >= 10.0 {
        1
    } else {
        2
    };
    let mut number = format!("{magnitude:.digits$}");
    if number.contains('.') {
        number = number
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned();
    }
    format!("{number}/s")
}

/// `iron-plate` -> `Iron plate` (the dump has no localisation, so we prettify the id).
pub(crate) fn display_name(id: &str) -> String {
    let mut name = id.replace('-', " ");
    if let Some(first) = name.get(..1) {
        name = first.to_uppercase() + &name[1..];
    }
    name
}

/// What goes where on the bar (in bar pixels).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Item {
    /// Section header (`Input:` / `Output:`).
    Header { text: &'static str, x: f32, y: f32 },
    /// Row: icon, name and rate per second.
    Row {
        flow: usize,
        icon_x: f32,
        y: f32,
        name: String,
        name_x: f32,
        amount: String,
        amount_x: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Bar {
    pub width: u32,
    pub height: u32,
    pub font_px: f32,
    pub icon: f32,
    pub items: Vec<Item>,
}

/// Lays out the list: `Input:` followed by one row per input, then `Output:` and the output
/// rows. Columns (icon, name, rate) are shared by both sections. The bar is at least
/// as wide as `min_width`, and grows when the longest row is wider.
pub(crate) fn layout(flows: &[Flow], min_width: u32, scale: u32) -> Option<Bar> {
    if flows.is_empty() {
        return None;
    }
    let font_px = (scale as f32 * 0.5).max(9.0);
    let icon = scale.max(10) as f32;
    let margin = (scale as f32 / 4.0).max(3.0);
    let row_height = icon + margin / 2.0;
    let header_height = font_px * 1.5 + margin / 2.0;

    let names: Vec<String> = flows.iter().map(|f| display_name(&f.name)).collect();
    let amounts: Vec<String> = flows.iter().map(|f| format_rate(f.per_second)).collect();
    let widest = |texts: &[String]| {
        texts
            .iter()
            .map(|t| text::width(t, font_px, Weight::Regular))
            .fold(0.0, f32::max)
    };
    let icon_x = margin;
    let name_x = icon_x + icon + margin;
    let amount_x = name_x + widest(&names) + margin * 2.0;
    let content = amount_x + widest(&amounts) + margin;
    let width = (min_width as f32).max(content).ceil() as u32;

    let mut items = Vec::new();
    let mut y = margin;
    let inputs = flows.iter().take_while(|f| f.is_input()).count();
    for (header, range) in [("Input:", 0..inputs), ("Output:", inputs..flows.len())] {
        if range.is_empty() {
            continue;
        }
        items.push(Item::Header {
            text: header,
            x: margin,
            y,
        });
        y += header_height;
        for flow in range {
            items.push(Item::Row {
                flow,
                icon_x,
                y,
                name: names[flow].clone(),
                name_x,
                amount: amounts[flow].clone(),
                amount_x,
            });
            y += row_height;
        }
        y += margin / 2.0;
    }
    Some(Bar {
        width,
        height: (y + margin / 2.0).ceil() as u32,
        font_px,
        icon,
        items,
    })
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

    #[test]
    fn rates_are_formatted_compactly_per_second() {
        assert_eq!(format_rate(-2.0), "2/s");
        assert_eq!(format_rate(0.25), "0.25/s");
        assert_eq!(format_rate(-12.34), "12.3/s");
        assert_eq!(format_rate(130.4), "130/s");
        assert_eq!(format_rate(1.0 / 3.0), "0.33/s");
    }

    #[test]
    fn ids_become_readable_names() {
        assert_eq!(display_name("iron-gear-wheel"), "Iron gear wheel");
        assert_eq!(display_name("coal"), "Coal");
        assert_eq!(display_name(""), "");
    }

    fn flow(name: &str, rate: f64) -> Flow {
        Flow {
            name: name.into(),
            per_second: rate,
        }
    }

    fn headers(bar: &Bar) -> Vec<&'static str> {
        bar.items
            .iter()
            .filter_map(|i| match i {
                Item::Header { text, .. } => Some(*text),
                Item::Row { .. } => None,
            })
            .collect()
    }

    #[test]
    fn layout_lists_inputs_then_outputs_one_per_line() {
        let flows = [flow("a", -1.0), flow("b", -2.0), flow("c", 3.0)];
        assert!(layout(&[], 500, 32).is_none());
        let bar = layout(&flows, 100, 32).unwrap();
        assert_eq!(headers(&bar), ["Input:", "Output:"]);
        let order: Vec<_> = bar
            .items
            .iter()
            .map(|i| match i {
                Item::Header { text, .. } => (*text).to_owned(),
                Item::Row { name, .. } => name.clone(),
            })
            .collect();
        assert_eq!(order, ["Input:", "A", "B", "Output:", "C"]);
        let ys: Vec<f32> = bar
            .items
            .iter()
            .map(|i| match i {
                Item::Header { y, .. } | Item::Row { y, .. } => *y,
            })
            .collect();
        assert!(
            ys.windows(2).all(|w| w[0] < w[1]),
            "every line is below the previous one"
        );
        assert!(bar.height as f32 > *ys.last().unwrap() + bar.icon);
    }

    #[test]
    fn columns_are_shared_and_the_bar_widens_for_long_rows() {
        let flows = [flow("short", -1.0), flow("a-much-longer-item-name", 1.0)];
        let bar = layout(&flows, 50, 32).unwrap();
        let columns: Vec<(f32, f32)> = bar
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Row {
                    name_x, amount_x, ..
                } => Some((*name_x, *amount_x)),
                Item::Header { .. } => None,
            })
            .collect();
        assert_eq!(columns[0], columns[1]);
        assert!(
            bar.width as f32 > columns[0].1,
            "amounts fit inside the bar"
        );
        assert!(layout(&flows, 5000, 32).unwrap().width >= 5000);
    }

    #[test]
    fn missing_sections_have_no_header() {
        let only_outputs = layout(&[flow("a", 1.0)], 500, 32).unwrap();
        assert_eq!(headers(&only_outputs), ["Output:"]);
        let only_inputs = layout(&[flow("a", -1.0)], 500, 32).unwrap();
        assert_eq!(headers(&only_inputs), ["Input:"]);
    }
}
