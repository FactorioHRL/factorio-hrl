use std::sync::Arc;

use crate::engine::{Bounds, Engine};
use crate::error::{Error, Result};
use crate::report::Report;
use crate::task::{Kind, Placement, Task};

/// A layout under construction for one task: machines are added one at a time and the set of
/// legal next placements is kept up to date incrementally, so a step costs roughly one
/// evaluation instead of re-validating the whole layout.
///
/// Per-kind grids (`legal`, `planes`) are indexed `kind * height * width + y * width + x`, like
/// [`Engine::legal_actions`].
pub struct Episode {
    engine: Arc<Engine>,
    task: Task,
    kinds: Vec<Kind>,
    bounds: Bounds,
    layout: Vec<Placement>,
    legal: Vec<bool>,
    planes: Vec<bool>,
}

impl Episode {
    /// An empty layout for `task`.
    pub fn new(engine: Arc<Engine>, task: Task) -> Result<Self> {
        let kinds = engine.kinds(&task)?;
        let bounds = engine.bounds(&task)?;
        let legal = engine.legal_actions(&task, &[])?;
        let planes = vec![false; legal.len()];
        Ok(Self {
            engine,
            task,
            kinds,
            bounds,
            layout: Vec::new(),
            legal,
            planes,
        })
    }

    pub fn task(&self) -> &Task {
        &self.task
    }

    /// The things that can be built; the index is the `kind` argument of [`Episode::place`].
    pub fn kinds(&self) -> &[Kind] {
        &self.kinds
    }

    pub fn layout(&self) -> &[Placement] {
        &self.layout
    }

    /// Whether a machine of each kind still fits at each tile (top left corner).
    pub fn legal(&self) -> &[bool] {
        &self.legal
    }

    /// Which tiles are covered by a machine of each kind.
    pub fn planes(&self) -> &[bool] {
        &self.planes
    }

    /// Builds a machine of `kind` with its top left tile at `(x, y)`.
    pub fn place(&mut self, kind: usize, x: i32, y: i32) -> Result<()> {
        let illegal = Error::IllegalPlacement { kind, x, y };
        let (width, height) = (self.task.width, self.task.height);
        let Some(chosen) = self.kinds.get(kind).cloned() else {
            return Err(illegal);
        };
        if x < 0 || y < 0 || x >= width || y >= height || !self.legal[self.index(kind, x, y)] {
            return Err(illegal);
        }
        for ty in y..y + chosen.height {
            for tx in x..x + chosen.width {
                let at = self.index(kind, tx, ty);
                self.planes[at] = true;
            }
        }
        // a machine of size (w, h) placed at (px, py) overlaps the new one when
        // px in (x - w, x + chosen.width) and py in (y - h, y + chosen.height)
        for other in 0..self.kinds.len() {
            let (w, h) = (self.kinds[other].width, self.kinds[other].height);
            for py in (y - h + 1).max(0)..(y + chosen.height).min(height) {
                for px in (x - w + 1).max(0)..(x + chosen.width).min(width) {
                    let at = self.index(other, px, py);
                    self.legal[at] = false;
                }
            }
        }
        self.layout.push(Placement {
            machine: chosen.machine,
            recipe: chosen.recipe,
            x,
            y,
        });
        Ok(())
    }

    /// Output, waste and the rest for the current layout. It is valid by construction, so no
    /// validation is repeated.
    pub fn report(&self) -> Result<Report> {
        self.engine
            .report(&self.task, &self.layout, Vec::new(), &self.bounds)
    }

    fn index(&self, kind: usize, x: i32, y: i32) -> usize {
        (kind * self.task.height as usize + y as usize) * self.task.width as usize + x as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsim_data::RawData;

    /// Two footprints (2×2 and 3×3) so the incremental mask is checked across different sizes.
    fn engine() -> Arc<Engine> {
        Arc::new(Engine::new(
            RawData::from_slice(
                br#"{
                "assembling-machine": {
                    "small": {"name": "small", "crafting_speed": 1, "crafting_categories": ["crafting"],
                        "collision_box": [[-0.9, -0.9], [0.9, 0.9]]},
                    "big": {"name": "big", "crafting_speed": 1, "crafting_categories": ["crafting"],
                        "collision_box": [[-1.2, -1.2], [1.2, 1.2]]}
                },
                "recipe": {
                    "cable": {"name": "cable", "energy_required": 1,
                        "ingredients": [{"name": "copper", "amount": 1}],
                        "results": [{"name": "cable", "amount": 2}]},
                    "circuit": {"name": "circuit", "energy_required": 1,
                        "ingredients": [{"name": "cable", "amount": 3}],
                        "results": [{"name": "circuit", "amount": 1}]}
                }
            }"#,
            )
            .unwrap(),
        ))
    }

    fn task() -> Task {
        Task {
            width: 11,
            height: 9,
            supply: [("copper".to_owned(), Some(5.0))].into(),
            recipes: vec!["cable".into(), "circuit".into()],
            machines: vec!["small".into(), "big".into()],
            target: "circuit".into(),
            required_output: None,
            require_access: false,
        }
    }

    #[test]
    fn the_incremental_mask_and_report_match_a_full_evaluation() {
        let engine = engine();
        let mut episode = Episode::new(engine.clone(), task()).unwrap();
        let mut seed = 12345u64;
        loop {
            let legal: Vec<usize> = (0..episode.legal().len())
                .filter(|&i| episode.legal()[i])
                .collect();
            if legal.is_empty() {
                break;
            }
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let pick = legal[(seed >> 33) as usize % legal.len()];
            let (kind, cell) = (pick / 99, pick % 99);
            episode
                .place(kind, (cell % 11) as i32, (cell / 11) as i32)
                .unwrap();

            let expected = engine.legal_actions(&task(), episode.layout()).unwrap();
            assert_eq!(episode.legal(), expected.as_slice());
            assert_eq!(
                episode.report().unwrap(),
                engine.evaluate(&task(), episode.layout()).unwrap()
            );
        }
        assert!(episode.layout().len() > 5, "the grid was filled");
    }

    #[test]
    fn plane_tiles_follow_the_placed_kinds() {
        let mut episode = Episode::new(engine(), task()).unwrap();
        let small = episode
            .kinds()
            .iter()
            .position(|k| k.machine == "small")
            .unwrap();
        episode.place(small, 1, 2).unwrap();
        let covered = episode.planes().iter().filter(|&&on| on).count();
        assert_eq!(covered, 4, "a 2x2 machine covers four tiles");
        assert!(episode.planes()[(small * 9 + 2) * 11 + 1]);
    }

    #[test]
    fn illegal_placements_are_rejected_and_change_nothing() {
        let mut episode = Episode::new(engine(), task()).unwrap();
        episode.place(0, 0, 0).unwrap();
        for (kind, x, y) in [(0, 0, 0), (0, 10, 8), (0, -1, 0), (99, 0, 0), (0, 11, 0)] {
            assert!(episode.place(kind, x, y).is_err(), "({kind}, {x}, {y})");
        }
        assert_eq!(episode.layout().len(), 1);
    }
}
