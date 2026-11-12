use std::collections::HashMap;

use fsim_blueprint::model::{
    CenterPos, Design, Direction, Entity, EntityName, IoType, Quality, Recipe,
};
use fsim_data::{EntityGeometry, RawData};

use crate::error::{Error, Result};

/// Tile canvas (top left corner = (0, 0)) with occupancy checks.
///
/// Every entity occupies its whole footprint: a 3×3 assembler is 9 tiles, a 2×2 furnace is 4,
/// a 2×1 splitter is 2 (1×2 after a quarter turn). Sizes come from `fsim-data`.
pub struct Canvas<'a> {
    data: &'a RawData,
    width: i32,
    height: i32,
    occupied: Vec<bool>,
    geometry: HashMap<String, EntityGeometry>,
    design: Design,
}

impl<'a> Canvas<'a> {
    pub fn new(data: &'a RawData, width: i32, height: i32) -> Self {
        Self {
            data,
            width,
            height,
            occupied: vec![false; (width * height) as usize],
            geometry: HashMap::new(),
            design: Design::default(),
        }
    }

    pub fn data(&self) -> &'a RawData {
        self.data
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn design(&self) -> &Design {
        &self.design
    }

    pub fn into_design(self) -> Design {
        self.design
    }

    /// Footprint size `(width, height)` in tiles, taking direction into account.
    pub fn size(&mut self, name: &str, direction: Direction) -> Result<(i32, i32)> {
        if !self.geometry.contains_key(name) {
            let info = self
                .data
                .entity_geometry(name)?
                .ok_or_else(|| Error::UnknownEntity(name.to_owned()))?;
            self.geometry.insert(name.to_owned(), info.geometry);
        }
        let quarter = (((direction.index() + 2) / 4) % 4) as u8;
        let g = self.geometry[name].oriented(quarter);
        Ok((g.width as i32, g.height as i32))
    }

    fn tile(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
    }

    pub fn is_free(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height && !self.occupied[self.tile(x, y)]
    }

    fn check(&mut self, name: &str, x: i32, y: i32, direction: Direction) -> Result<(i32, i32)> {
        let (w, h) = self.size(name, direction)?;
        let fail = |reason| Error::Placement {
            name: name.to_owned(),
            x,
            y,
            reason,
        };
        if x < 0 || y < 0 || x + w > self.width || y + h > self.height {
            return Err(fail("outside the area"));
        }
        for ty in y..y + h {
            for tx in x..x + w {
                if self.occupied[self.tile(tx, ty)] {
                    return Err(fail("tile already occupied"));
                }
            }
        }
        Ok((w, h))
    }

    /// Whether the entity fits with the footprint's top left corner at tile `(x, y)`.
    pub fn fits(&mut self, name: &str, x: i32, y: i32, direction: Direction) -> bool {
        self.check(name, x, y, direction).is_ok()
    }

    /// Places an entity. `(x, y)` is the footprint's top left tile (already rotated).
    /// Returns the entity index in `design`.
    pub fn place(&mut self, name: &str, x: i32, y: i32, direction: Direction) -> Result<usize> {
        let (w, h) = self.check(name, x, y, direction)?;
        for ty in y..y + h {
            for tx in x..x + w {
                let i = self.tile(tx, ty);
                self.occupied[i] = true;
            }
        }
        self.design.entities.push(Entity {
            name: EntityName::new(name),
            // footprint center in doubled coordinates
            pos: CenterPos::new(2 * x + w, 2 * y + h),
            direction,
            mirror: false,
            quality: Quality::normal(),
            recipe: None,
            modules: Vec::new(),
            io: None,
        });
        Ok(self.design.entities.len() - 1)
    }

    /// Like `place`, but returns `false` instead of an error when the spot is occupied or off the canvas.
    pub fn place_if_free(&mut self, name: &str, x: i32, y: i32, direction: Direction) -> bool {
        self.place(name, x, y, direction).is_ok()
    }

    pub fn set_recipe(&mut self, index: usize, recipe: &str) {
        self.design.entities[index].recipe = Some(Recipe {
            name: recipe.into(),
            quality: Quality::normal(),
        });
    }

    pub fn set_io(&mut self, index: usize, io: IoType) {
        self.design.entities[index].io = Some(io);
    }

    pub fn set_mirror(&mut self, index: usize, mirror: bool) {
        self.design.entities[index].mirror = mirror;
    }
}

/// Rotates the whole design a quarter turn clockwise.
/// `height` is the canvas height before the rotation, and its width after it.
pub fn rotate_cw(design: &mut Design, height: i32) {
    for e in &mut design.entities {
        e.pos = CenterPos::new(2 * height - e.pos.y2, e.pos.x2);
        e.direction = e.direction.rotate_cw(4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> RawData {
        let json = serde_json_like();
        RawData::from_slice(json.as_bytes()).unwrap()
    }

    fn serde_json_like() -> String {
        r#"{
            "furnace": {"stone-furnace": {"type":"furnace","name":"stone-furnace","collision_box":[[-0.7,-0.7],[0.7,0.7]]}},
            "assembling-machine": {"assembling-machine-2": {"type":"assembling-machine","name":"assembling-machine-2","collision_box":[[-1.2,-1.2],[1.2,1.2]]}},
            "splitter": {"splitter": {"type":"splitter","name":"splitter","collision_box":[[-0.9,-0.4],[0.9,0.4]]}},
            "transport-belt": {"transport-belt": {"type":"transport-belt","name":"transport-belt","collision_box":[[-0.4,-0.4],[0.4,0.4]]}}
        }"#
        .to_owned()
    }

    #[test]
    fn footprints_block_every_tile_they_cover() {
        let data = data();
        let mut c = Canvas::new(&data, 8, 8);
        c.place("assembling-machine-2", 1, 1, Direction::NORTH)
            .unwrap();
        for y in 1..4 {
            for x in 1..4 {
                assert!(!c.is_free(x, y), "({x},{y}) should be taken by the 3x3");
            }
        }
        assert!(c.is_free(4, 1) && c.is_free(0, 1) && c.is_free(1, 4));
        assert!(c.place("transport-belt", 3, 3, Direction::EAST).is_err());
        assert!(c.place("transport-belt", 4, 3, Direction::EAST).is_ok());

        c.place("stone-furnace", 5, 5, Direction::NORTH).unwrap();
        assert!(!c.is_free(5, 5) && !c.is_free(6, 6));
        assert!(c.is_free(7, 5) && c.is_free(5, 7));
    }

    #[test]
    fn centre_is_the_middle_of_the_footprint() {
        let data = data();
        let mut c = Canvas::new(&data, 8, 8);
        let furnace = c.place("stone-furnace", 2, 4, Direction::NORTH).unwrap();
        let machine = c
            .place("assembling-machine-2", 4, 0, Direction::NORTH)
            .unwrap();
        let e = &c.design().entities;
        assert_eq!((e[furnace].pos.x(), e[furnace].pos.y()), (3.0, 5.0));
        assert_eq!((e[machine].pos.x(), e[machine].pos.y()), (5.5, 1.5));
    }

    #[test]
    fn splitter_swaps_its_footprint_when_turned() {
        let data = data();
        let mut c = Canvas::new(&data, 4, 4);
        assert_eq!(c.size("splitter", Direction::NORTH).unwrap(), (2, 1));
        assert_eq!(c.size("splitter", Direction::EAST).unwrap(), (1, 2));
        c.place("splitter", 1, 1, Direction::EAST).unwrap();
        assert!(!c.is_free(1, 1) && !c.is_free(1, 2));
        assert!(c.is_free(2, 1) && c.is_free(0, 1));
    }

    #[test]
    fn rejects_out_of_bounds_and_unknown() {
        let data = data();
        let mut c = Canvas::new(&data, 4, 4);
        assert!(c
            .place("assembling-machine-2", 2, 0, Direction::NORTH)
            .is_err());
        assert!(c
            .place("assembling-machine-2", -1, 0, Direction::NORTH)
            .is_err());
        assert!(matches!(
            c.place("nope", 0, 0, Direction::NORTH),
            Err(Error::UnknownEntity(_))
        ));
        assert!(c.design().entities.is_empty());
    }

    #[test]
    fn rotation_keeps_footprints_on_the_grid() {
        let data = data();
        let mut c = Canvas::new(&data, 6, 4);
        c.place("assembling-machine-2", 0, 0, Direction::NORTH)
            .unwrap();
        c.place("splitter", 4, 1, Direction::EAST).unwrap();
        let mut design = c.into_design();
        rotate_cw(&mut design, 4);
        // a 3x3 in the top left corner lands in the top right corner of the 4x6 canvas
        assert_eq!(
            (design.entities[0].pos.x(), design.entities[0].pos.y()),
            (2.5, 1.5)
        );
        // a 1x2 splitter (E) becomes 2x1 (S) at tile (1, 4)
        assert_eq!(design.entities[1].direction, Direction::SOUTH);
        assert_eq!(
            (design.entities[1].pos.x(), design.entities[1].pos.y()),
            (2.0, 4.5)
        );
    }
}
