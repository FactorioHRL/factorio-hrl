use std::collections::{HashMap, HashSet};

use fsim_blueprint::model::{Design, Entity};
use fsim_data::{ItemKind, RawData};
use serde_json::Value;

use crate::error::Result;
use crate::grid::step;
use crate::layout::{quarter_turns, Placed};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Opening {
    pub x: i32,
    pub y: i32,
    pub side: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RecipeFluids {
    pub inputs: Vec<Option<u32>>,
    pub outputs: Vec<Option<u32>>,
}

impl RecipeFluids {
    fn side(&self, production: &str) -> &[Option<u32>] {
        if production == "input" {
            &self.inputs
        } else {
            &self.outputs
        }
    }
}

fn port_in_use(fluids: &[Option<u32>], ordinal: usize) -> bool {
    if fluids.iter().any(Option::is_some) {
        return fluids.contains(&Some(ordinal as u32 + 1));
    }
    fluids.len() == 1 || ordinal < fluids.len()
}

const BOX_KEYS: [&str; 4] = [
    "fluid_box",
    "fluid_boxes",
    "input_fluid_box",
    "output_fluid_box",
];

fn pair(value: &Value) -> Option<(f64, f64)> {
    match value {
        Value::Array(xy) => Some((xy.first()?.as_f64()?, xy.get(1)?.as_f64()?)),
        Value::Object(xy) => Some((xy.get("x")?.as_f64()?, xy.get("y")?.as_f64()?)),
        _ => None,
    }
}

fn is_active(
    fluid_box: &Value,
    ordinals: &mut HashMap<String, usize>,
    prototype: &Value,
    recipe: Option<&RecipeFluids>,
) -> bool {
    let Some(production) = fluid_box.get("production_type").and_then(Value::as_str) else {
        return true;
    };
    if prototype.get("crafting_categories").is_none() || !matches!(production, "input" | "output") {
        return true;
    }
    let ordinal = ordinals.entry(production.to_owned()).or_insert(0);
    let index = *ordinal;
    *ordinal += 1;
    match recipe {
        Some(fluids) => port_in_use(fluids.side(production), index),
        None => !prototype
            .get("fluid_boxes_off_when_no_fluid_recipe")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

pub(crate) fn openings(
    entity: &Entity,
    placed: &Placed,
    prototype: &Value,
    recipe: Option<&RecipeFluids>,
) -> Vec<Opening> {
    let turns = quarter_turns(entity.direction);
    let mut ordinals = HashMap::new();
    let mut out = Vec::new();

    for key in BOX_KEYS {
        let boxes = match prototype.get(key) {
            Some(Value::Array(list)) => list.iter().collect(),
            Some(single @ Value::Object(_)) => vec![single],
            _ => continue,
        };
        for fluid_box in boxes {
            if !is_active(fluid_box, &mut ordinals, prototype, recipe) {
                continue;
            }
            let connections = fluid_box.get("pipe_connections").and_then(Value::as_array);
            for connection in connections.into_iter().flatten() {
                if let Some(kind) = connection.get("connection_type").and_then(Value::as_str) {
                    if kind != "normal" {
                        continue;
                    }
                }
                let (Some((mut x, mut y)), Some(direction)) = (
                    connection.get("position").and_then(pair),
                    connection.get("direction").and_then(Value::as_u64),
                ) else {
                    continue;
                };
                let mut direction = direction as u8;
                if entity.mirror {
                    x = -x;
                    direction = (16 - direction % 16) % 16;
                }
                for _ in 0..turns {
                    (x, y) = (-y, x);
                }
                out.push(Opening {
                    x: (placed.cx + x).floor() as i32,
                    y: (placed.cy + y).floor() as i32,
                    side: ((direction + 2) / 4 + turns) % 4,
                });
            }
        }
    }
    out
}

pub(crate) struct Connections {
    openings: HashSet<Opening>,
}

fn recipe_fluids(data: &RawData, name: &str) -> Result<Option<RecipeFluids>> {
    Ok(data.recipe(name)?.map(|recipe| RecipeFluids {
        inputs: recipe
            .ingredients
            .iter()
            .filter(|i| i.kind == ItemKind::Fluid)
            .map(|i| i.fluidbox_index)
            .collect(),
        outputs: recipe
            .results
            .iter()
            .filter(|p| p.kind == ItemKind::Fluid)
            .map(|p| p.fluidbox_index)
            .collect(),
    }))
}

impl Connections {
    pub fn new(design: &Design, placed: &[Placed], data: &RawData) -> Result<Self> {
        let mut cache: HashMap<String, Option<RecipeFluids>> = HashMap::new();
        let mut set = HashSet::new();
        for (entity, p) in design.entities.iter().zip(placed) {
            let Some(prototype) = data.get(&p.kind, &p.name) else {
                continue;
            };
            let fluids = match &entity.recipe {
                None => None,
                Some(recipe) => {
                    if !cache.contains_key(&*recipe.name) {
                        cache.insert(recipe.name.to_string(), recipe_fluids(data, &recipe.name)?);
                    }
                    cache[&*recipe.name].as_ref()
                }
            };
            set.extend(openings(entity, p, prototype, fluids));
        }
        Ok(Self { openings: set })
    }

    pub fn faces(&self, x: i32, y: i32, side: u8) -> bool {
        let (dx, dy) = step(side);
        self.openings.contains(&Opening {
            x: x + dx,
            y: y + dy,
            side: (side + 2) % 4,
        })
    }
}

#[cfg(test)]
mod tests {
    use fsim_blueprint::model::{CenterPos, Direction, EntityName, Quality};
    use serde_json::json;

    use super::*;

    fn entity(direction: Direction, mirror: bool) -> Entity {
        Entity {
            name: EntityName::new("x"),
            pos: CenterPos::new(1, 1),
            direction,
            mirror,
            quality: Quality::normal(),
            recipe: None,
            modules: Vec::new(),
            io: None,
        }
    }

    fn placed() -> Placed {
        Placed {
            name: "x".into(),
            kind: "x".into(),
            direction: Direction::NORTH,
            io: None,
            cx: 0.5,
            cy: 0.5,
            width: 1.0,
            height: 1.0,
        }
    }

    fn fluids(inputs: &[Option<u32>], outputs: &[Option<u32>]) -> RecipeFluids {
        RecipeFluids {
            inputs: inputs.to_vec(),
            outputs: outputs.to_vec(),
        }
    }

    fn targets(
        prototype: &Value,
        direction: Direction,
        mirror: bool,
        recipe: Option<&RecipeFluids>,
    ) -> Vec<(i32, i32)> {
        let mut out: Vec<_> = openings(&entity(direction, mirror), &placed(), prototype, recipe)
            .into_iter()
            .map(|o| {
                let (dx, dy) = step(o.side);
                (o.x + dx, o.y + dy)
            })
            .collect();
        out.sort_unstable();
        out
    }

    fn expect(relative: &[(i32, i32)]) -> Vec<(i32, i32)> {
        let mut out: Vec<_> = relative.to_vec();
        out.sort_unstable();
        out
    }

    fn port(direction: u8, x: f64, y: f64) -> Value {
        json!({"direction": direction, "position": [x, y]})
    }

    fn chemical_plant() -> Value {
        json!({"crafting_categories": ["chemistry"], "fluid_boxes": [
            {"production_type": "input", "pipe_connections": [port(0, -1.0, -1.0)]},
            {"production_type": "input", "pipe_connections": [port(0, 1.0, -1.0)]},
            {"production_type": "output", "pipe_connections": [port(8, -1.0, 1.0)]},
            {"production_type": "output", "pipe_connections": [port(8, 1.0, 1.0)]}
        ]})
    }

    fn refinery() -> Value {
        json!({"crafting_categories": ["oil-processing"], "fluid_boxes": [
            {"production_type": "input", "pipe_connections": [port(8, -1.0, 2.0)]},
            {"production_type": "input", "pipe_connections": [port(8, 1.0, 2.0)]},
            {"production_type": "output", "pipe_connections": [port(0, -2.0, -2.0)]},
            {"production_type": "output", "pipe_connections": [port(0, 0.0, -2.0)]},
            {"production_type": "output", "pipe_connections": [port(0, 2.0, -2.0)]}
        ]})
    }

    #[test]
    fn single_fluid_recipe_connects_every_port_of_that_side() {
        let plastic = fluids(&[None], &[]);
        let north = targets(&chemical_plant(), Direction::NORTH, false, Some(&plastic));
        assert_eq!(north, expect(&[(-1, -2), (1, -2)]));
        let east_mirrored = targets(&chemical_plant(), Direction::EAST, true, Some(&plastic));
        assert_eq!(east_mirrored, expect(&[(2, -1), (2, 1)]));
        let south = targets(&chemical_plant(), Direction::SOUTH, false, Some(&plastic));
        assert_eq!(south, expect(&[(-1, 2), (1, 2)]));
    }

    #[test]
    fn two_fluid_inputs_and_one_output() {
        let cracking = fluids(&[None, None], &[None]);
        let all = expect(&[(-1, -2), (1, -2), (-1, 2), (1, 2)]);
        for mirror in [false, true] {
            assert_eq!(
                targets(&chemical_plant(), Direction::NORTH, mirror, Some(&cracking)),
                all
            );
        }
        let east = expect(&[(2, -1), (2, 1), (-2, -1), (-2, 1)]);
        assert_eq!(
            targets(&chemical_plant(), Direction::EAST, true, Some(&cracking)),
            east
        );
    }

    #[test]
    fn machine_without_recipe_exposes_all_ports() {
        let all = expect(&[(-1, -2), (1, -2), (-1, 2), (1, 2)]);
        assert_eq!(
            targets(&chemical_plant(), Direction::NORTH, false, None),
            all
        );
    }

    #[test]
    fn ports_can_be_switched_off_without_a_recipe() {
        let mut plant = chemical_plant();
        plant["fluid_boxes_off_when_no_fluid_recipe"] = json!(true);
        assert!(targets(&plant, Direction::NORTH, false, None).is_empty());
    }

    #[test]
    fn recipe_without_fluids_has_no_ports() {
        let gears = fluids(&[], &[]);
        assert!(targets(&chemical_plant(), Direction::NORTH, false, Some(&gears)).is_empty());
    }

    #[test]
    fn explicit_fluidbox_index_selects_the_port_and_mirroring_flips_it() {
        let basic = fluids(&[Some(2)], &[Some(3)]);
        let at = |direction, mirror| targets(&refinery(), direction, mirror, Some(&basic));
        assert_eq!(at(Direction::NORTH, false), expect(&[(1, 3), (2, -3)]));
        assert_eq!(at(Direction::NORTH, true), expect(&[(-1, 3), (-2, -3)]));
        assert_eq!(at(Direction::EAST, true), expect(&[(-3, -1), (3, -2)]));
    }

    #[test]
    fn underground_connections_are_not_surface_openings() {
        let pipe_to_ground = json!({"fluid_box": {"pipe_connections": [
            port(0, 0.0, 0.0),
            {"direction": 8, "position": [0, 0], "connection_type": "underground"}
        ]}});
        assert_eq!(
            targets(&pipe_to_ground, Direction::NORTH, false, None),
            expect(&[(0, -1)])
        );
        assert_eq!(
            targets(&pipe_to_ground, Direction::EAST, false, None),
            expect(&[(1, 0)])
        );
    }

    #[test]
    fn storage_tank_and_plain_pipe() {
        let tank = json!({"fluid_box": {"pipe_connections": [
            port(0, -1.0, -1.0), port(4, 1.0, 1.0), port(8, 1.0, 1.0), port(12, -1.0, -1.0)
        ]}});
        let north = expect(&[(-1, -2), (2, 1), (1, 2), (-2, -1)]);
        assert_eq!(targets(&tank, Direction::NORTH, false, None), north);
        let east = expect(&[(2, -1), (-1, 2), (-2, 1), (1, -2)]);
        assert_eq!(targets(&tank, Direction::EAST, false, None), east);

        let pipe = json!({"fluid_box": {"pipe_connections": [
            port(0, 0.0, 0.0), port(4, 0.0, 0.0), port(8, 0.0, 0.0), port(12, 0.0, 0.0)
        ]}});
        assert_eq!(
            targets(&pipe, Direction::NORTH, false, None),
            expect(&[(0, -1), (1, 0), (0, 1), (-1, 0)])
        );
    }

    #[test]
    fn port_selection_rules() {
        assert!(port_in_use(&[None], 0) && port_in_use(&[None], 1));
        assert!(port_in_use(&[None, None], 1) && !port_in_use(&[None, None], 2));
        assert!(!port_in_use(&[], 0));
        assert!(port_in_use(&[Some(2)], 1) && !port_in_use(&[Some(2)], 0));
    }

    #[test]
    fn facing_openings_connect_in_both_directions() {
        let (a, b) = (
            Opening {
                x: 0,
                y: 0,
                side: 1,
            },
            Opening {
                x: 1,
                y: 0,
                side: 3,
            },
        );
        let connections = Connections {
            openings: [a, b].into_iter().collect(),
        };
        assert!(connections.faces(0, 0, 1));
        assert!(connections.faces(1, 0, 3));
        assert!(!connections.faces(0, 0, 0));
        assert!(!connections.faces(0, 0, 3));
    }
}
