use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use fsim_blueprint::model::{CenterPos, Design, Direction, Entity, Quality, Recipe};
use fsim_data::{Memo, RawData};

use crate::access::{access, ratios, Access};
use crate::error::Result;
use crate::links::{link_ratio, Chain};
use crate::report::{far_tiles, FlowReport, GroupReport, PlacedView, Report, Violation};
use crate::shape::shape;
use crate::task::{Kind, Placement, Task};

/// Footprint of a machine in tiles.
type Size = (i32, i32);

/// Holds the loaded game data and answers questions about layouts.
pub struct Engine {
    data: RawData,
    /// What the grid allows depends on the task only, not on the layout, and costs a linear
    /// programme: solved once per task. Training samples many tasks, hence the bound.
    bounds: Memo<Bounds>,
}

/// What a task's grid allows, and which recipes feed which, whatever the layout.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Bounds {
    /// Most the grid could make of the target (the required output when that is lower).
    pub ceiling: f64,
    /// Machines of the smallest allowed footprint that fit: the scale idle machines are
    /// measured against.
    pub machines: usize,
    /// The recipes of the task that feed each other.
    pub chain: Arc<Chain>,
}

/// Distinct tasks remembered before the table of bounds is emptied.
const BOUNDS_MEMO_LIMIT: usize = 4096;

impl Engine {
    pub fn new(data: RawData) -> Self {
        Self {
            data,
            bounds: Memo::with_limit(BOUNDS_MEMO_LIMIT),
        }
    }

    /// Loads `data-raw-dump.json`.
    pub fn load(dump: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::new(RawData::load(dump)?))
    }

    /// Every allowed machine that can craft an allowed recipe: the choices an agent has.
    /// The order is stable (task order of machines, then recipes), so indices can be used as
    /// action ids.
    pub fn kinds(&self, task: &Task) -> Result<Vec<Kind>> {
        task.check()?;
        let mut kinds = Vec::new();
        for machine in &task.machines {
            let (Some(info), Some(machine_data)) = (
                self.data.entity_geometry(machine)?,
                self.data.machine(machine)?,
            ) else {
                continue;
            };
            for recipe in &task.recipes {
                let Some(recipe_data) = self.data.recipe(recipe)? else {
                    continue;
                };
                let craftable = recipe_data
                    .categories
                    .iter()
                    .any(|c| machine_data.crafting_categories.contains(c));
                if craftable {
                    kinds.push(Kind {
                        machine: machine.clone(),
                        recipe: recipe.clone(),
                        width: info.geometry.width as i32,
                        height: info.geometry.height as i32,
                    });
                }
            }
        }
        Ok(kinds)
    }

    /// Checks a layout against the task. Empty means valid.
    pub fn validate(&self, task: &Task, layout: &[Placement]) -> Result<Vec<Violation>> {
        task.check()?;
        let mut violations = Vec::new();
        let mut sizes: HashMap<&str, Option<Size>> = HashMap::new();
        let mut owner: HashMap<(i32, i32), usize> = HashMap::new();
        for (index, p) in layout.iter().enumerate() {
            if !task.machines.contains(&p.machine) {
                violations.push(Violation::MachineNotAllowed {
                    index,
                    name: p.machine.clone(),
                });
            }
            if !task.recipes.contains(&p.recipe) {
                violations.push(Violation::RecipeNotAllowed {
                    index,
                    name: p.recipe.clone(),
                });
            }
            let size = match sizes.get(p.machine.as_str()) {
                Some(size) => *size,
                None => {
                    let size = self.footprint(&p.machine)?;
                    sizes.insert(&p.machine, size);
                    size
                }
            };
            let Some((w, h)) = size else {
                violations.push(Violation::UnknownPrototype {
                    index,
                    name: p.machine.clone(),
                });
                continue;
            };
            if !self.can_craft(&p.machine, &p.recipe)? {
                violations.push(Violation::CannotCraft {
                    index,
                    machine: p.machine.clone(),
                    recipe: p.recipe.clone(),
                });
            }
            if p.x < 0 || p.y < 0 || p.x + w > task.width || p.y + h > task.height {
                violations.push(Violation::OutOfBounds { index });
                continue;
            }
            for tile in tiles(p.x, p.y, w, h) {
                match owner.insert(tile, index) {
                    Some(other) if other != index => {
                        violations.push(Violation::Overlap { a: other, b: index });
                        break;
                    }
                    _ => {}
                }
            }
        }
        Ok(violations)
    }

    /// Validates the layout and computes what it produces.
    pub fn evaluate(&self, task: &Task, layout: &[Placement]) -> Result<Report> {
        let violations = self.validate(task, layout)?;
        let bounds = self.bounds(task)?;
        self.report(task, layout, violations, &bounds)
    }

    /// Validates and scores the machines of a blueprint string (entities that are not machines
    /// with a recipe on a whole tile are reported as violations).
    pub fn analyze_blueprint(&self, task: &Task, text: &str) -> Result<Report> {
        let root = fsim_blueprint::decode(text)?;
        let design = Design::from_root(&root)?
            .into_iter()
            .next()
            .unwrap_or_default();
        let mut found = Vec::new();
        let mut violations = Vec::new();
        for (index, entity) in design.entities.iter().enumerate() {
            match self.placement_of(entity)? {
                Some(placement) => found.push(placement),
                None => violations.push(Violation::UnsupportedEntity {
                    index,
                    name: entity.name.to_string(),
                }),
            }
        }
        // blueprints use arbitrary coordinates: shift so the top left machine sits at (0, 0)
        let min_x = found.iter().map(|p| p.x).min().unwrap_or(0);
        let min_y = found.iter().map(|p| p.y).min().unwrap_or(0);
        let placements: Vec<Placement> = found
            .into_iter()
            .map(|p| Placement {
                x: p.x - min_x,
                y: p.y - min_y,
                ..p
            })
            .collect();
        violations.extend(self.validate(task, &placements)?);
        let bounds = self.bounds(task)?;
        self.report(task, &placements, violations, &bounds)
    }

    /// The layout as a blueprint string, to paste into the game.
    pub fn export_blueprint(&self, layout: &[Placement]) -> Result<String> {
        Ok(self.design(layout)?.to_blueprint_string()?)
    }

    /// For every kind (see [`Engine::kinds`]) and tile, whether a machine of that kind fits
    /// there without overlapping anything. Indexed `kind * height * width + y * width + x`.
    pub fn legal_actions(&self, task: &Task, layout: &[Placement]) -> Result<Vec<bool>> {
        let kinds = self.kinds(task)?;
        let (width, height) = (task.width as usize, task.height as usize);
        let mut occupied = vec![false; width * height];
        for p in layout {
            if let Some((w, h)) = self.footprint(&p.machine)? {
                for (x, y) in tiles(p.x, p.y, w, h) {
                    if let Some(tile) = occupied.get_mut(y as usize * width + x as usize) {
                        *tile = true;
                    }
                }
            }
        }
        let mut legal = Vec::with_capacity(kinds.len() * width * height);
        for kind in &kinds {
            for y in 0..task.height {
                for x in 0..task.width {
                    let fits = x + kind.width <= task.width
                        && y + kind.height <= task.height
                        && tiles(x, y, kind.width, kind.height)
                            .all(|(tx, ty)| !occupied[ty as usize * width + tx as usize]);
                    legal.push(fits);
                }
            }
        }
        Ok(legal)
    }

    /// `bounds` are [`Engine::bounds`] of the task, passed in so an [`crate::Episode`] computes them once.
    pub(crate) fn report(
        &self,
        task: &Task,
        layout: &[Placement],
        violations: Vec<Violation>,
        bounds: &Bounds,
    ) -> Result<Report> {
        let ceiling = bounds.ceiling;
        let valid = violations.is_empty();
        let mut placed = Vec::with_capacity(layout.len());
        for (index, p) in layout.iter().enumerate() {
            let (width, height) = self.footprint(&p.machine)?.unwrap_or((0, 0));
            placed.push(PlacedView {
                index,
                machine: p.machine.clone(),
                recipe: p.recipe.clone(),
                x: p.x,
                y: p.y,
                width,
                height,
            });
        }
        let tiles_used = placed.iter().map(|p| i64::from(p.width * p.height)).sum();
        let tiles_total = i64::from(task.width) * i64::from(task.height);

        let access = access(&placed, task.width, task.height);
        let working: Cow<[Placement]> = if task.require_access {
            let reachable = layout
                .iter()
                .zip(&access)
                .filter(|(_, a)| **a == Access::Open);
            Cow::Owned(reachable.map(|(p, _)| p.clone()).collect())
        } else {
            Cow::Borrowed(layout)
        };
        let (output, groups, flows) = if valid {
            let state = fsim_calc::steady_state(
                &self.design(&working)?,
                &self.data,
                &task.supply,
                &task.target,
                task.required_output,
            )?;
            let groups = state
                .groups
                .iter()
                .map(|g| GroupReport {
                    machine: g.machine.clone(),
                    recipe: g.recipe.clone(),
                    count: g.count,
                    crafts_per_second: g.crafts_per_second,
                    utilisation: g.utilisation(),
                })
                .collect::<Vec<_>>();
            let flows = state
                .flows
                .into_iter()
                .map(|f| FlowReport {
                    item: f.name,
                    per_second: f.per_second,
                })
                .collect();
            (state.target_per_second, groups, flows)
        } else {
            (0.0, Vec::new(), Vec::new())
        };
        let machines = layout.len();
        // machines that cannot work (see `Task::require_access`) are idle too
        let idle: f64 = groups
            .iter()
            .map(|g| g.count as f64 * (1.0 - g.utilisation))
            .sum::<f64>()
            + (layout.len() - working.len()) as f64;
        let shape = shape(&placed, task.width, task.height);
        let (enclosed, cut_off) = ratios(&access, bounds.machines);
        let link = link_ratio(
            &placed,
            &bounds.chain,
            far_tiles(task.width, task.height),
            bounds.machines,
        );
        Ok(Report {
            valid,
            violations,
            placed,
            machines,
            tiles_used,
            tiles_total,
            output_per_second: output,
            ceiling_per_second: ceiling,
            output_ratio: if ceiling > 1e-9 {
                (output / ceiling).min(1.0)
            } else {
                0.0
            },
            idle_machines: idle,
            // idle machines against the grid's machine budget, not against the machines built so
            // far: while a chain is still incomplete most of what is built idles, and measured
            // against itself that reads as "everything is waste" after the very first machine
            waste_ratio: if bounds.machines > 0 {
                (idle / bounds.machines as f64).min(1.0)
            } else {
                0.0
            },
            adjacency_ratio: shape.adjacency,
            line_ratio: shape.line,
            stray_ratio: shape.stray,
            enclosed_ratio: enclosed,
            cut_off_ratio: cut_off,
            link_ratio: link,
            groups,
            flows,
        })
    }

    /// Output ceiling and machine budget of the task. The budget is the number of the smallest
    /// allowed footprint that fit (exact packing when all footprints are equal).
    pub(crate) fn bounds(&self, task: &Task) -> Result<Bounds> {
        let key = serde_json::to_string(task).expect("a task is plain data and always serialises");
        let bounds = self
            .bounds
            .get_or_compute(&key, || self.solve_bounds(task).map(Some))?;
        Ok(bounds.expect("the closure always yields a value"))
    }

    fn solve_bounds(&self, task: &Task) -> Result<Bounds> {
        let tiles_total = i64::from(task.width) * i64::from(task.height);
        let chain = Arc::new(Chain::new(&self.data, &task.recipes)?);
        let kinds = self.kinds(task)?;
        let Some(smallest) = kinds.iter().min_by_key(|k| k.width * k.height) else {
            return Ok(Bounds {
                ceiling: 0.0,
                machines: 0,
                chain,
            });
        };
        let same_size = kinds
            .iter()
            .all(|k| (k.width, k.height) == (smallest.width, smallest.height));
        let budget = if same_size {
            i64::from(task.width / smallest.width) * i64::from(task.height / smallest.height)
        } else {
            tiles_total / i64::from(smallest.width * smallest.height)
        };
        let pairs: Vec<_> = kinds
            .iter()
            .map(|k| (k.machine.clone(), k.recipe.clone()))
            .collect();
        let grid_ceiling = fsim_calc::ceiling(
            &self.data,
            &pairs,
            budget as usize,
            &task.supply,
            &task.target,
        )?;
        Ok(Bounds {
            ceiling: task
                .required_output
                .map_or(grid_ceiling, |required| required.min(grid_ceiling)),
            machines: budget as usize,
            chain,
        })
    }

    fn can_craft(&self, machine: &str, recipe: &str) -> Result<bool> {
        let (Some(machine), Some(recipe)) =
            (self.data.machine(machine)?, self.data.recipe(recipe)?)
        else {
            return Ok(false);
        };
        Ok(recipe
            .categories
            .iter()
            .any(|c| machine.crafting_categories.contains(c)))
    }

    fn footprint(&self, machine: &str) -> Result<Option<Size>> {
        Ok(self
            .data
            .entity_geometry(machine)?
            .map(|i| (i.geometry.width as i32, i.geometry.height as i32)))
    }

    /// The loaded game data.
    pub fn data(&self) -> &RawData {
        &self.data
    }

    /// The layout as a model design (all machines face north).
    pub fn design(&self, layout: &[Placement]) -> Result<Design> {
        let mut entities = Vec::with_capacity(layout.len());
        for p in layout {
            let (w, h) = self.footprint(&p.machine)?.unwrap_or((1, 1));
            entities.push(Entity {
                name: p.machine.as_str().into(),
                pos: CenterPos::new(2 * p.x + w, 2 * p.y + h),
                direction: Direction::NORTH,
                mirror: false,
                quality: Quality::normal(),
                recipe: Some(Recipe {
                    name: p.recipe.as_str().into(),
                    quality: Quality::normal(),
                }),
                modules: Vec::new(),
                io: None,
            });
        }
        Ok(Design { entities })
    }

    /// The placement of a blueprint entity, or `None` when it is not a recipe machine on a whole
    /// tile.
    fn placement_of(&self, entity: &Entity) -> Result<Option<Placement>> {
        let (Some(recipe), Some((w, h))) = (&entity.recipe, self.footprint(&entity.name)?) else {
            return Ok(None);
        };
        let (x2, y2) = (entity.pos.x2 - w, entity.pos.y2 - h);
        if x2 % 2 != 0 || y2 % 2 != 0 {
            return Ok(None);
        }
        Ok(Some(Placement {
            machine: entity.name.to_string(),
            recipe: recipe.name.to_string(),
            x: x2 / 2,
            y: y2 / 2,
        }))
    }
}

/// Tiles of a `w`×`h` rectangle with top left corner `(x, y)`.
fn tiles(x: i32, y: i32, w: i32, h: i32) -> impl Iterator<Item = (i32, i32)> {
    (y..y + h).flat_map(move |ty| (x..x + w).map(move |tx| (tx, ty)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3×3 assembler that crafts at speed 0.5, so cable and circuit recipes (0.5 s) run at
    /// one craft per second.
    fn engine() -> Engine {
        Engine::new(
            RawData::from_slice(
                br#"{
                "assembling-machine": {
                    "asm": {"name": "asm", "crafting_speed": 0.5, "crafting_categories": ["crafting"],
                        "collision_box": [[-1.2, -1.2], [1.2, 1.2]]},
                    "chem": {"name": "chem", "crafting_speed": 1, "crafting_categories": ["chemistry"],
                        "collision_box": [[-1.2, -1.2], [1.2, 1.2]]}
                },
                "recipe": {
                    "cable": {"name": "cable", "energy_required": 0.5,
                        "ingredients": [{"name": "copper", "amount": 1}],
                        "results": [{"name": "cable", "amount": 2}]},
                    "circuit": {"name": "circuit", "energy_required": 0.5,
                        "ingredients": [{"name": "iron", "amount": 1}, {"name": "cable", "amount": 3}],
                        "results": [{"name": "circuit", "amount": 1}]},
                    "plastic": {"name": "plastic", "category": "chemistry",
                        "ingredients": [], "results": [{"name": "plastic", "amount": 1}]}
                }
            }"#,
            )
            .unwrap(),
        )
    }

    fn task(width: i32, height: i32, iron: f64, copper: f64) -> Task {
        Task {
            width,
            height,
            supply: [
                ("iron".to_owned(), Some(iron)),
                ("copper".to_owned(), Some(copper)),
            ]
            .into(),
            recipes: vec!["cable".into(), "circuit".into()],
            machines: vec!["asm".into()],
            target: "circuit".into(),
            required_output: None,
            require_access: false,
        }
    }

    fn at(recipe: &str, x: i32, y: i32) -> Placement {
        Placement {
            machine: "asm".into(),
            recipe: recipe.into(),
            x,
            y,
        }
    }

    /// `cables` cable machines then `circuits` circuit machines, packed in a row of 3×3 cells.
    fn row(cables: i32, circuits: i32) -> Vec<Placement> {
        (0..cables)
            .map(|i| at("cable", i * 3, 0))
            .chain((0..circuits).map(|i| at("circuit", (cables + i) * 3, 0)))
            .collect()
    }

    #[test]
    fn the_target_layout_uses_all_the_supply() {
        // 10 iron/s and 15 copper/s need 15 cable and 10 circuit machines
        let report = engine()
            .evaluate(&task(75, 3, 10.0, 15.0), &row(15, 10))
            .unwrap();
        assert!(report.valid, "{:?}", report.violations);
        assert!((report.output_per_second - 10.0).abs() < 1e-6);
        assert!((report.output_ratio - 1.0).abs() < 1e-6);
        assert!(report.idle_machines < 1e-6);
    }

    #[test]
    fn surplus_machines_count_as_waste_but_not_as_output() {
        let report = engine()
            .evaluate(&task(90, 3, 10.0, 15.0), &row(18, 12))
            .unwrap();
        assert!((report.output_per_second - 10.0).abs() < 1e-6);
        assert!(
            (report.idle_machines - 5.0).abs() < 1e-6,
            "{}",
            report.idle_machines
        );
        assert!(report.waste_ratio > 0.1);
    }

    #[test]
    fn machines_nobody_can_reach_make_nothing_when_the_task_asks_for_access() {
        let e = engine();
        // 3x3 slots of 3x3 machines on a 10x10 grid: the middle slot is walled in by the others
        let layout: Vec<Placement> = (0..9)
            .map(|i| {
                let recipe = if i == 4 { "circuit" } else { "cable" };
                at(recipe, (i % 3) * 3, (i / 3) * 3)
            })
            .collect();
        let mut t = task(10, 10, 10.0, 15.0);
        let free = e.evaluate(&t, &layout).unwrap();
        assert!(
            (free.output_per_second - 1.0).abs() < 1e-6,
            "the walled-in circuit machine works"
        );
        t.require_access = true;
        let gated = e.evaluate(&t, &layout).unwrap();
        assert_eq!(
            gated.output_per_second, 0.0,
            "the only circuit machine cannot be reached"
        );
        assert!(
            gated.idle_machines >= free.idle_machines + 1.0,
            "an unreachable machine is idle"
        );
    }

    #[test]
    fn waste_is_measured_against_the_grid_not_against_what_is_built() {
        let e = engine();
        let t = task(30, 30, 10.0, 15.0); // 100 machines fit
        let half_built = e.evaluate(&t, &row(1, 0)).unwrap(); // a lone cable machine idles
        assert!((half_built.idle_machines - 1.0).abs() < 1e-6);
        assert!(
            (half_built.waste_ratio - 0.01).abs() < 1e-9,
            "one idle machine is 1% of the grid, not 100% of the layout"
        );
    }

    #[test]
    fn the_remembered_ceiling_follows_the_task() {
        let e = engine();
        let rich = task(30, 30, 10.0, 15.0);
        let poor = task(30, 30, 1.0, 15.0);
        let first = e.evaluate(&rich, &[]).unwrap().ceiling_per_second;
        let other = e.evaluate(&poor, &[]).unwrap().ceiling_per_second;
        assert!(other < first, "less iron, lower ceiling");
        assert_eq!(e.evaluate(&rich, &[]).unwrap().ceiling_per_second, first);
    }

    #[test]
    fn a_required_output_is_the_target_and_extra_machines_are_waste() {
        let e = engine();
        let mut asked = task(90, 3, 0.0, 0.0); // one row of 30 machines
        asked.supply = [("iron".to_owned(), None), ("copper".to_owned(), None)].into();
        asked.required_output = Some(5.0);
        let exact = e.evaluate(&asked, &row(8, 5)).unwrap(); // 5 circuit/s need 5 + 7.5 machines
        assert!((exact.output_per_second - 5.0).abs() < 1e-6);
        assert!((exact.ceiling_per_second - 5.0).abs() < 1e-6);
        assert!((exact.output_ratio - 1.0).abs() < 1e-6);
        let spare = e.evaluate(&asked, &row(15, 10)).unwrap();
        assert!(
            (spare.output_per_second - 5.0).abs() < 1e-6,
            "no more than asked"
        );
        assert!(spare.waste_ratio > exact.waste_ratio);
        let short = e.evaluate(&asked, &row(8, 3)).unwrap();
        assert!((short.output_ratio - 0.6).abs() < 1e-6);
        asked.required_output = Some(0.0);
        assert!(e.evaluate(&asked, &[]).is_err());
    }

    #[test]
    fn ceiling_is_limited_by_the_grid_when_it_is_small() {
        // a 9x3 grid holds 3 machines: c cable + k circuit with k = 2c/3 -> c = 1.8, k = 1.2
        let report = engine().evaluate(&task(9, 3, 10.0, 15.0), &[]).unwrap();
        assert!((report.ceiling_per_second - 1.2).abs() < 1e-6);
        assert_eq!(report.output_per_second, 0.0);
    }

    #[test]
    fn violations_are_reported_and_zero_the_output() {
        let layout = [
            at("cable", 0, 0),
            at("cable", 2, 0),   // overlaps the first
            at("circuit", 9, 0), // out of bounds on a 10-wide grid
            at("plastic", 3, 3), // recipe not allowed (and not craftable by asm)
        ];
        let report = engine().evaluate(&task(10, 6, 1.0, 1.0), &layout).unwrap();
        assert!(!report.valid);
        assert_eq!(report.output_per_second, 0.0);
        assert!(report
            .violations
            .contains(&Violation::Overlap { a: 0, b: 1 }));
        assert!(report
            .violations
            .contains(&Violation::OutOfBounds { index: 2 }));
        assert!(report.violations.contains(&Violation::RecipeNotAllowed {
            index: 3,
            name: "plastic".into()
        }));
        assert!(report.violations.contains(&Violation::CannotCraft {
            index: 3,
            machine: "asm".into(),
            recipe: "plastic".into()
        }));
    }

    #[test]
    fn legal_actions_exclude_occupied_and_edge_tiles() {
        let task = task(6, 3, 1.0, 1.0);
        let engine = engine();
        let empty = engine.legal_actions(&task, &[]).unwrap();
        // 2 kinds x 3 rows x 6 columns; a 3x3 fits only with x in 0..=3, y = 0
        assert_eq!(empty.len(), 2 * 3 * 6);
        assert_eq!(empty[..18].iter().filter(|l| **l).count(), 4);
        let after = engine.legal_actions(&task, &[at("cable", 0, 0)]).unwrap();
        assert_eq!(after[..18].iter().filter(|l| **l).count(), 1); // only x = 3 remains
        assert!(after[3]);
    }

    #[test]
    fn blueprint_round_trip_matches_the_layout() {
        let engine = engine();
        let layout = row(4, 3);
        let text = engine.export_blueprint(&layout).unwrap();
        let from_text = engine
            .analyze_blueprint(&task(21, 3, 10.0, 15.0), &text)
            .unwrap();
        let direct = engine.evaluate(&task(21, 3, 10.0, 15.0), &layout).unwrap();
        assert!(from_text.valid, "{:?}", from_text.violations);
        assert_eq!(from_text.placed, direct.placed);
        assert!((from_text.output_per_second - direct.output_per_second).abs() < 1e-9);
    }

    #[test]
    fn invalid_tasks_are_errors() {
        assert!(engine().evaluate(&task(0, 3, 1.0, 1.0), &[]).is_err());
        assert!(engine().evaluate(&task(3, 3, -1.0, 1.0), &[]).is_err());
    }

    #[test]
    fn unlimited_supply_reports_the_draw_of_the_layout() {
        let mut task = task(75, 3, 0.0, 0.0);
        task.supply = [("iron".to_owned(), None), ("copper".to_owned(), None)].into();
        let report = engine().evaluate(&task, &row(15, 10)).unwrap();
        assert!((report.output_per_second - 10.0).abs() < 1e-6);
        let draw = |item: &str| {
            report
                .flows
                .iter()
                .find(|f| f.item == item)
                .map(|f| -f.per_second)
        };
        assert!((draw("iron").unwrap() - 10.0).abs() < 1e-6);
        assert!((draw("copper").unwrap() - 15.0).abs() < 1e-6);
    }
}
