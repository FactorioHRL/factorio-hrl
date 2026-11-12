//! Building blocks of layouts: belt lanes with splitters, machine rows, the petrochemical block.

use fsim_blueprint::model::{Direction, IoType};

use crate::canvas::Canvas;
use crate::error::{Error, Result};
use crate::rng::Rng;

const ASSEMBLERS: [&str; 3] = [
    "assembling-machine-1",
    "assembling-machine-2",
    "assembling-machine-3",
];
const ASSEMBLER_RECIPES: [&str; 12] = [
    "iron-gear-wheel",
    "copper-cable",
    "electronic-circuit",
    "pipe",
    "inserter",
    "transport-belt",
    "engine-unit",
    "firearm-magazine",
    "automation-science-pack",
    "logistic-science-pack",
    "underground-belt",
    "splitter",
];
const CHEMICAL_RECIPES: [&str; 6] = [
    "plastic-bar",
    "sulfur",
    "sulfuric-acid",
    "solid-fuel-from-petroleum-gas",
    "lubricant",
    "light-oil-cracking",
];
const REFINERY_RECIPES: [&str; 2] = ["basic-oil-processing", "advanced-oil-processing"];
const FURNACES: [&str; 2] = ["stone-furnace", "steel-furnace"];
const INSERTERS: [&str; 2] = ["inserter", "fast-inserter"];
const POLES: [&str; 2] = ["small-electric-pole", "medium-electric-pole"];

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    /// 3×3 machines spaced every 4 tiles (poles in the gaps).
    Assembler,
    /// 2×2 furnaces next to each other.
    Furnace,
}

impl Kind {
    fn height(self) -> i32 {
        match self {
            Kind::Assembler => 3,
            Kind::Furnace => 2,
        }
    }
}

/// Recipes from the pool that the given machine can craft.
fn compatible<'a>(c: &Canvas, machine: &str, pool: &[&'a str]) -> Result<Vec<&'a str>> {
    let data = c.data();
    let categories = data
        .machine(machine)?
        .ok_or_else(|| Error::UnknownEntity(machine.to_owned()))?
        .crafting_categories;
    let mut ok = Vec::new();
    for &name in pool {
        if let Some(recipe) = data.recipe(name)? {
            if recipe.categories.iter().any(|cat| categories.contains(cat)) {
                ok.push(name);
            }
        }
    }
    if ok.is_empty() {
        return Err(Error::NoRecipe(machine.to_owned()));
    }
    Ok(ok)
}

fn dir_for(rng: &mut Rng, horizontal: bool) -> Direction {
    match (horizontal, rng.chance(0.5)) {
        (true, true) => Direction::EAST,
        (true, false) => Direction::WEST,
        (false, true) => Direction::NORTH,
        (false, false) => Direction::SOUTH,
    }
}

/// Splitter across a lane: for a belt running E/W it occupies 1×2 tiles and spans two
/// adjacent rows.
fn splitters(c: &mut Canvas, rng: &mut Rng, y: i32, dir: Direction, fast: bool) {
    let name = if fast { "fast-splitter" } else { "splitter" };
    for _ in 0..rng.range(1, 3) {
        let x = rng.range(3, c.width() - 3);
        c.place_if_free(name, x, y, dir);
    }
}

const CHESTS: [&str; 3] = ["wooden-chest", "iron-chest", "steel-chest"];

/// A pair of underground belts on a lane running E/W. Entrance and exit are `gap` tiles apart
/// (max 4: `max_distance` 5 minus the exit tile), and chests stand above the tunnel,
/// so an inserter from the adjacent row still has somewhere to drop.
fn underground_pair(c: &mut Canvas, rng: &mut Rng, y: i32, dir: Direction, fast: bool) {
    let name = if fast {
        "fast-underground-belt"
    } else {
        "underground-belt"
    };
    let gap = rng.range(1, 5);
    let left = rng.range(1, c.width() - gap - 2);
    let right = left + gap + 1;
    let (entry, exit) = if dir == Direction::EAST {
        (left, right)
    } else {
        (right, left)
    };
    let free = (left..=right).all(|x| c.is_free(x, y));
    if !free {
        return;
    }
    let input = c.place(name, entry, y, dir).expect("checked free");
    c.set_io(input, IoType::Input);
    let output = c.place(name, exit, y, dir).expect("checked free");
    c.set_io(output, IoType::Output);
    for x in left + 1..right {
        c.place(rng.pick(&CHESTS), x, y, Direction::NORTH)
            .expect("checked free");
    }
}

/// `n` parallel lanes with the same direction, across the full width. Splitters stand
/// between adjacent lanes and the remaining tiles are belts.
fn belt_group(c: &mut Canvas, rng: &mut Rng, y: i32, n: i32, dir: Direction) {
    let fast = rng.chance(0.4);
    for r in 0..n - 1 {
        splitters(c, rng, y + r, dir, fast);
    }
    if n == 1 {
        for _ in 0..rng.range(0, 3) {
            underground_pair(c, rng, y, dir, fast);
        }
    }
    let belt = if fast {
        "fast-transport-belt"
    } else {
        "transport-belt"
    };
    for r in 0..n {
        for x in 0..c.width() {
            c.place_if_free(belt, x, y + r, dir);
        }
    }
}

/// Lane + repeated (inserter row, machines, inserter row, lane), across the full
/// width. Lanes between bands are shared, so the output of one band feeds the next.
/// Leftover rows (when the height does not divide evenly) turn into extra parallel
/// lanes with splitters.
pub(crate) fn bands(
    c: &mut Canvas,
    rng: &mut Rng,
    y0: i32,
    rows: i32,
    kind: Kind,
    first_dir: Direction,
) -> Result<()> {
    let h = kind.height();
    let period = h + 3;
    let count = (rows - 1) / period;
    if count < 1 {
        return Err(Error::TooSmall {
            width: c.width(),
            height: rows,
            min: period + 1,
        });
    }
    let mut extra = vec![0; count as usize + 1];
    for _ in 0..rows - 1 - count * period {
        extra[rng.range(0, count + 1) as usize] += 1;
    }

    let mut y = y0;
    for g in 0..=count {
        let dir = if g % 2 == 0 {
            first_dir
        } else {
            first_dir.opposite()
        };
        let lanes = 1 + extra[g as usize];
        belt_group(c, rng, y, lanes, dir);
        y += lanes;
        if g < count {
            match kind {
                Kind::Assembler => assembler_band(c, rng, y, h)?,
                Kind::Furnace => furnace_band(c, rng, y, h)?,
            }
            y += h + 2;
        }
    }
    Ok(())
}

fn inserter_row(c: &mut Canvas, rng: &mut Rng, x: i32, y: i32, columns: i32) {
    let mask = rng.range(1, 1 << columns);
    for col in 0..columns {
        if mask & (1 << col) != 0 {
            let name = *rng.pick(&INSERTERS);
            c.place_if_free(name, x + col, y, Direction::SOUTH);
        }
    }
}

fn assembler_band(c: &mut Canvas, rng: &mut Rng, y: i32, h: i32) -> Result<()> {
    let machine = *rng.pick(&ASSEMBLERS);
    let recipes = compatible(c, machine, &ASSEMBLER_RECIPES)?;
    let base = *rng.pick(&recipes);
    let (top, mid, bottom) = (y, y + 1, y + 1 + h);
    let mut x = 0;
    while x + 3 <= c.width() {
        if rng.chance(0.08) {
            // empty slot
        } else if rng.chance(0.08) {
            c.place("beacon", x, mid, Direction::NORTH)?;
        } else {
            let id = c.place(machine, x, mid, Direction::NORTH)?;
            let recipe = if rng.chance(0.75) {
                base
            } else {
                *rng.pick(&recipes)
            };
            c.set_recipe(id, recipe);
            inserter_row(c, rng, x, top, 3);
            inserter_row(c, rng, x, bottom, 3);
        }
        if x + 3 < c.width() {
            c.place_if_free(rng.pick(&POLES), x + 3, mid + 1, Direction::NORTH);
        }
        x += 4;
    }
    Ok(())
}

fn furnace_band(c: &mut Canvas, rng: &mut Rng, y: i32, h: i32) -> Result<()> {
    let base = *rng.pick(&FURNACES);
    let (top, mid, bottom) = (y, y + 1, y + 1 + h);
    let mut x = 0;
    let mut index = 0;
    while x + 2 <= c.width() {
        if index % 4 == 1 {
            c.place_if_free("small-electric-pole", x + 1, top, Direction::NORTH);
        }
        if !rng.chance(0.1) {
            let name = if rng.chance(0.85) {
                base
            } else {
                *rng.pick(&FURNACES)
            };
            c.place(name, x, mid, Direction::NORTH)?;
            let inserter = *rng.pick(&INSERTERS);
            c.place_if_free(inserter, x, top, Direction::SOUTH);
            if rng.chance(0.3) {
                c.place_if_free(inserter, x + 1, top, Direction::SOUTH);
            }
            c.place_if_free(inserter, x + rng.range(0, 2), bottom, Direction::SOUTH);
        }
        x += 2;
        index += 1;
    }
    Ok(())
}

/// Petrochemical block in `rows` rows from `y0`: refineries, a pipe row, chemical plants
/// with inserters and an output lane. Height 11, because that is what these parts take.
pub(crate) const OIL_ROWS: i32 = 11;

pub(crate) fn oil_block(c: &mut Canvas, rng: &mut Rng, y0: i32) -> Result<()> {
    let w = c.width();
    let refinery_recipes = compatible(c, "oil-refinery", &REFINERY_RECIPES)?;
    let mut x = 0;
    while x + 5 <= w - 3 {
        let id = c.place("oil-refinery", x, y0, Direction::NORTH)?;
        c.set_recipe(id, rng.pick(&refinery_recipes));
        x += 6;
    }
    // tank in the top right corner, connected to a pipe
    c.place("storage-tank", w - 3, y0, Direction::NORTH)?;
    for y in y0 + 3..y0 + 5 {
        c.place_if_free("pipe", w - 2, y, Direction::NORTH);
    }
    for x in 0..w {
        c.place_if_free("pipe", x, y0 + 5, Direction::NORTH);
    }

    let chem_recipes = compatible(c, "chemical-plant", &CHEMICAL_RECIPES)?;
    let plants = y0 + 6;
    let mut x = 0;
    while x + 3 <= w {
        let facing = if rng.chance(0.5) {
            Direction::NORTH
        } else {
            Direction::SOUTH
        };
        let id = c.place("chemical-plant", x, plants, facing)?;
        c.set_recipe(id, rng.pick(&chem_recipes));
        c.set_mirror(id, rng.chance(0.3));
        if rng.chance(0.7) {
            let inserter = *rng.pick(&INSERTERS);
            c.place_if_free(inserter, x + 1, plants + 3, Direction::SOUTH);
        }
        let gap = x + 3;
        if gap < w {
            if rng.chance(0.5) {
                for y in plants..plants + 3 {
                    c.place_if_free("pipe", gap, y, Direction::NORTH);
                }
            } else {
                c.place_if_free(rng.pick(&POLES), gap, plants + 1, Direction::NORTH);
            }
        }
        x += 4;
    }
    Ok(())
}

/// Horizontal lane on the last row of the petrochemical block (`y0 + OIL_ROWS - 1`).
pub(crate) fn oil_belt(c: &mut Canvas, rng: &mut Rng, y: i32, dir: Direction) {
    belt_group(c, rng, y, 1, dir);
}

pub(crate) fn random_horizontal(rng: &mut Rng) -> Direction {
    dir_for(rng, true)
}
