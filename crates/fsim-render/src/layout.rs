use fsim_blueprint::model::{Design, Direction, IoType};
use fsim_data::{EntityGeometry, RawData};

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Placed {
    pub name: String,
    pub kind: String,
    pub direction: Direction,
    pub io: Option<IoType>,
    pub cx: f64,
    pub cy: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

pub(crate) fn quarter_turns(direction: Direction) -> u8 {
    (((direction.index() + 2) / 4) % 4) as u8
}

pub(crate) fn place(design: &Design, data: &RawData) -> Result<Vec<Placed>> {
    let mut known: std::collections::HashMap<&str, (String, EntityGeometry)> =
        std::collections::HashMap::new();
    let mut placed = Vec::with_capacity(design.entities.len());
    for entity in &design.entities {
        let name: &str = &entity.name;
        if !known.contains_key(name) {
            let info = data
                .entity_geometry(name)?
                .ok_or_else(|| Error::UnknownEntity(name.to_owned()))?;
            known.insert(name, (info.kind, info.geometry));
        }
        let (kind, geometry) = &known[name];
        let geometry = geometry.oriented(quarter_turns(entity.direction));
        placed.push(Placed {
            name: name.to_owned(),
            kind: kind.clone(),
            direction: entity.direction,
            io: entity.io,
            cx: entity.pos.x(),
            cy: entity.pos.y(),
            width: f64::from(geometry.width),
            height: f64::from(geometry.height),
        });
    }
    Ok(placed)
}

pub(crate) fn bounds(placed: &[Placed]) -> Result<Bounds> {
    let first = placed.first().ok_or(Error::EmptyDesign)?;
    let mut b = Bounds {
        min_x: first.cx - first.width / 2.0,
        min_y: first.cy - first.height / 2.0,
        max_x: first.cx + first.width / 2.0,
        max_y: first.cy + first.height / 2.0,
    };
    for p in &placed[1..] {
        b.min_x = b.min_x.min(p.cx - p.width / 2.0);
        b.min_y = b.min_y.min(p.cy - p.height / 2.0);
        b.max_x = b.max_x.max(p.cx + p.width / 2.0);
        b.max_y = b.max_y.max(p.cy + p.height / 2.0);
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_turns_round_to_nearest_cardinal() {
        assert_eq!(quarter_turns(Direction::NORTH), 0);
        assert_eq!(quarter_turns(Direction::EAST), 1);
        assert_eq!(quarter_turns(Direction::SOUTH), 2);
        assert_eq!(quarter_turns(Direction::WEST), 3);
        assert_eq!(quarter_turns(Direction::new(15).unwrap()), 0);
        assert_eq!(quarter_turns(Direction::new(2).unwrap()), 1);
    }
}
