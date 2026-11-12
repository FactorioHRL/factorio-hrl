use std::collections::HashMap;

use fsim_blueprint::model::{Design, Direction, IoType};
use fsim_data::RawData;

use crate::canvas::Canvas;
use crate::error::{Error, Result};
use crate::rng::{pick, Rng, RngExt, SeedableRng};
use crate::Params;

/// Prototype types to sample from (placeable, with graphics, ordinary entities).
const KINDS: [&str; 31] = [
    "transport-belt",
    "underground-belt",
    "splitter",
    "lane-splitter",
    "loader",
    "inserter",
    "assembling-machine",
    "furnace",
    "lab",
    "beacon",
    "container",
    "logistic-container",
    "pipe",
    "pipe-to-ground",
    "pump",
    "storage-tank",
    "offshore-pump",
    "electric-pole",
    "mining-drill",
    "boiler",
    "generator",
    "radar",
    "wall",
    "solar-panel",
    "accumulator",
    "roboport",
    "reactor",
    "ammo-turret",
    "electric-turret",
    "lamp",
    "heat-pipe",
];

/// Largest footprint side in tiles. Filters out e.g. the rocket silo (9×9).
const MAX_SIDE: i32 = 5;

/// How many sampling attempts per entity before the canvas is considered too crowded.
const ATTEMPTS_PER_ENTITY: usize = 200;

struct Candidate {
    name: String,
    kind: String,
}

/// All entities that can be sampled: one per prototype (each equally
/// likely), without junk like the Factorio logo and without giants.
fn candidates(c: &mut Canvas) -> Result<Vec<Candidate>> {
    let data = c.data();
    let mut found = Vec::new();
    for kind in KINDS {
        let mut names: Vec<&str> = data
            .of_kind(kind)
            .filter(|(name, value)| {
                !name.starts_with("factorio-logo")
                    && value
                        .get("flags")
                        .and_then(|f| f.as_array())
                        .is_some_and(|f| f.iter().any(|x| x == "player-creation"))
            })
            .map(|(name, _)| name)
            .collect();
        names.sort_unstable();
        for name in names {
            let (w, h) = c.size(name, Direction::NORTH)?;
            if w <= MAX_SIDE && h <= MAX_SIDE {
                found.push(Candidate {
                    name: name.to_owned(),
                    kind: kind.to_owned(),
                });
            }
        }
    }
    Ok(found)
}

/// Non-hidden recipes matching the machine's crafting categories.
fn recipes_by_machine(
    data: &RawData,
    machines: &[&Candidate],
) -> Result<HashMap<String, Vec<String>>> {
    let mut all = Vec::new();
    for (name, value) in data.of_kind("recipe") {
        if value.get("hidden").and_then(|h| h.as_bool()) == Some(true) {
            continue;
        }
        if let Some(recipe) = data.recipe(name)? {
            all.push(recipe);
        }
    }
    all.sort_by(|a, b| a.name.cmp(&b.name));
    let mut by_machine = HashMap::new();
    for m in machines {
        let Some(machine) = data.machine(&m.name)? else {
            continue;
        };
        let fitting: Vec<String> = all
            .iter()
            .filter(|r| {
                r.categories
                    .iter()
                    .any(|c| machine.crafting_categories.contains(c))
            })
            .map(|r| r.name.clone())
            .collect();
        by_machine.insert(m.name.clone(), fitting);
    }
    Ok(by_machine)
}

const DIRECTIONS: [Direction; 4] = [
    Direction::NORTH,
    Direction::EAST,
    Direction::SOUTH,
    Direction::WEST,
];

/// Random entities at random places: picks a random entity, direction and tile,
/// places it if it fits, and repeats up to `params.entities`. The same seed gives the same result.
pub fn generate(data: &RawData, params: Params) -> Result<Design> {
    let mut rng = Rng::seed_from_u64(params.seed);
    let mut canvas = Canvas::new(data, params.width, params.height);
    let pool = candidates(&mut canvas)?;
    let machines: Vec<&Candidate> = pool
        .iter()
        .filter(|c| c.kind == "assembling-machine")
        .collect();
    let recipes = recipes_by_machine(data, &machines)?;

    let mut placed = 0;
    let mut attempts = 0;
    while placed < params.entities && attempts < params.entities * ATTEMPTS_PER_ENTITY {
        attempts += 1;
        let entity = pick(&mut rng, &pool);
        let direction = *pick(&mut rng, &DIRECTIONS);
        let (w, h) = canvas.size(&entity.name, direction)?;
        if w > params.width || h > params.height {
            continue;
        }
        let x = rng.random_range(0..params.width - w + 1);
        let y = rng.random_range(0..params.height - h + 1);
        let Ok(index) = canvas.place(&entity.name, x, y, direction) else {
            continue;
        };
        if let Some(options) = recipes.get(&entity.name).filter(|r| !r.is_empty()) {
            canvas.set_recipe(index, pick(&mut rng, options));
        }
        if matches!(entity.kind.as_str(), "underground-belt" | "loader") {
            let io = if rng.random_bool(0.5) {
                IoType::Input
            } else {
                IoType::Output
            };
            canvas.set_io(index, io);
        }
        placed += 1;
    }
    if placed < params.entities {
        return Err(Error::TooCrowded {
            placed,
            wanted: params.entities,
        });
    }
    Ok(canvas.into_design())
}

/// Names of all entities that `generate` samples from (one per prototype).
pub fn candidate_names(data: &RawData) -> Result<Vec<String>> {
    let mut canvas = Canvas::new(data, 1, 1);
    Ok(candidates(&mut canvas)?
        .into_iter()
        .map(|c| c.name)
        .collect())
}
