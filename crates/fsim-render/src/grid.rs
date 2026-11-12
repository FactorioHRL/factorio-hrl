use std::collections::HashMap;

use crate::layout::Placed;

pub(crate) fn step(quarter: u8) -> (i32, i32) {
    match quarter % 4 {
        0 => (0, -1),
        1 => (1, 0),
        2 => (0, 1),
        _ => (-1, 0),
    }
}

pub(crate) struct TileGrid {
    tiles: HashMap<(i32, i32), usize>,
}

impl TileGrid {
    pub fn new(placed: &[Placed]) -> Self {
        let mut tiles = HashMap::new();
        for (index, p) in placed.iter().enumerate() {
            let left = (p.cx - p.width / 2.0).round() as i32;
            let top = (p.cy - p.height / 2.0).round() as i32;
            for x in left..left + p.width as i32 {
                for y in top..top + p.height as i32 {
                    tiles.insert((x, y), index);
                }
            }
        }
        Self { tiles }
    }

    pub fn at(&self, x: i32, y: i32) -> Option<usize> {
        self.tiles.get(&(x, y)).copied()
    }
}

#[cfg(test)]
mod tests {
    use fsim_blueprint::model::Direction;

    use super::*;

    fn placed(cx: f64, cy: f64, width: f64, height: f64) -> Placed {
        Placed {
            name: "x".into(),
            kind: "x".into(),
            direction: Direction::NORTH,
            io: None,
            cx,
            cy,
            width,
            height,
        }
    }

    #[test]
    fn odd_sized_entity_covers_its_tiles() {
        let grid = TileGrid::new(&[placed(1.5, 1.5, 3.0, 3.0)]);
        assert_eq!(grid.at(0, 0), Some(0));
        assert_eq!(grid.at(2, 2), Some(0));
        assert_eq!(grid.at(3, 1), None);
        assert_eq!(grid.at(-1, 1), None);
    }

    #[test]
    fn even_sized_entity_with_integer_centre() {
        let grid = TileGrid::new(&[placed(0.0, 0.5, 2.0, 1.0)]);
        assert_eq!(grid.at(-1, 0), Some(0));
        assert_eq!(grid.at(0, 0), Some(0));
        assert_eq!(grid.at(1, 0), None);
        assert_eq!(grid.at(0, 1), None);
    }

    #[test]
    fn negative_coordinates() {
        let grid = TileGrid::new(&[placed(-94.5, -192.5, 1.0, 1.0)]);
        assert_eq!(grid.at(-95, -193), Some(0));
        assert_eq!(grid.at(-94, -193), None);
    }
}
