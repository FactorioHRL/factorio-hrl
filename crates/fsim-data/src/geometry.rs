use serde::Deserialize;

use crate::error::{Error, Result};
use crate::raw::{Prototype, RawData};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityGeometry {
    pub width: u32,
    pub height: u32,
}

impl EntityGeometry {
    pub fn oriented(self, quarter_turns: u8) -> Self {
        if quarter_turns % 2 == 1 {
            Self {
                width: self.height,
                height: self.width,
            }
        } else {
            self
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityInfo {
    pub kind: String,
    pub name: String,
    pub geometry: EntityGeometry,
}

#[derive(Deserialize)]
struct GeometryDef {
    tile_width: Option<u32>,
    tile_height: Option<u32>,
    collision_box: Option<Vec<serde_json::Value>>,
}

fn point(value: &serde_json::Value) -> Option<[f64; 2]> {
    match value {
        serde_json::Value::Array(xy) => Some([xy.first()?.as_f64()?, xy.get(1)?.as_f64()?]),
        serde_json::Value::Object(xy) => Some([xy.get("x")?.as_f64()?, xy.get("y")?.as_f64()?]),
        _ => None,
    }
}

fn corners(boxed: &[serde_json::Value]) -> Option<([f64; 2], [f64; 2])> {
    Some((point(boxed.first()?)?, point(boxed.get(1)?)?))
}

fn tiles_covered(min: f64, max: f64) -> u32 {
    ((max - min) - 1e-6).ceil().max(1.0) as u32
}

fn convert(proto: Prototype<'_>) -> Result<EntityInfo> {
    let def: GeometryDef = proto.parse()?;
    let from_box = def
        .collision_box
        .as_deref()
        .and_then(corners)
        .map(|([x0, y0], [x1, y1])| (tiles_covered(x0, x1), tiles_covered(y0, y1)));
    let (width, height) = match (def.tile_width, def.tile_height, from_box) {
        (Some(w), Some(h), _) => (w, h),
        (w, h, Some((bw, bh))) => (w.unwrap_or(bw), h.unwrap_or(bh)),
        _ => {
            return Err(Error::NoCollisionBox {
                kind: proto.kind.to_owned(),
                name: proto.name.to_owned(),
            })
        }
    };
    Ok(EntityInfo {
        kind: proto.kind.to_owned(),
        name: proto.name.to_owned(),
        geometry: EntityGeometry { width, height },
    })
}

impl RawData {
    pub fn entity_geometry(&self, name: &str) -> Result<Option<EntityInfo>> {
        self.all()
            .find(|p| p.name == name && p.has("collision_box"))
            .map(convert)
            .transpose()
    }

    pub fn entity_geometries(&self) -> impl Iterator<Item = Result<EntityInfo>> + '_ {
        self.all().filter(|p| p.has("collision_box")).map(convert)
    }
}
