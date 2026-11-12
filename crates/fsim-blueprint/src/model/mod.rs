mod direction;
mod items;
mod names;

pub use direction::{Direction, DirectionScale};
pub use items::ItemStack;
pub use names::{EntityName, ItemName, Quality, RecipeName};

use crate::raw;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ModelError {
    #[error("entity {entity}: position ({x}, {y}) is not on the half-tile grid")]
    OffGrid { entity: u64, x: f64, y: f64 },
    #[error("entity {entity}: invalid direction {direction}")]
    InvalidDirection { entity: u64, direction: u8 },
    #[error("entity {entity}: invalid module/item data: {reason}")]
    InvalidItems { entity: u64, reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CenterPos {
    pub x2: i32,
    pub y2: i32,
}

impl CenterPos {
    pub fn new(x2: i32, y2: i32) -> Self {
        Self { x2, y2 }
    }

    pub fn from_f64(x: f64, y: f64) -> Option<Self> {
        Some(Self::new(doubled(x)?, doubled(y)?))
    }

    pub fn x(self) -> f64 {
        f64::from(self.x2) / 2.0
    }

    pub fn y(self) -> f64 {
        f64::from(self.y2) / 2.0
    }
}

fn doubled(v: f64) -> Option<i32> {
    let d = v * 2.0;
    (d.is_finite() && d.fract() == 0.0 && d.abs() < f64::from(i32::MAX)).then_some(d as i32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct GameVersion {
    major: u16,
}

impl GameVersion {
    fn unpack(packed: u64) -> Self {
        Self {
            major: (packed >> 48) as u16,
        }
    }

    fn direction_scale(self) -> DirectionScale {
        if self.major >= 2 {
            DirectionScale::Sixteen
        } else {
            DirectionScale::Eight
        }
    }
}

/// Side of an underground belt or loader: entrance or exit.
/// In the blueprint this is the `type` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoType {
    Input,
    Output,
}

impl IoType {
    pub fn as_str(self) -> &'static str {
        match self {
            IoType::Input => "input",
            IoType::Output => "output",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "input" => Some(IoType::Input),
            "output" => Some(IoType::Output),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub name: RecipeName,
    pub quality: Quality,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub name: EntityName,
    pub pos: CenterPos,
    pub direction: Direction,
    pub mirror: bool,
    pub quality: Quality,
    pub recipe: Option<Recipe>,
    pub modules: Vec<ItemStack>,
    /// `Some` only for entities with an input/output `type` field (underground belt, loader).
    pub io: Option<IoType>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Design {
    pub entities: Vec<Entity>,
}

impl Design {
    pub fn from_raw(bp: &raw::Blueprint) -> Result<Self, ModelError> {
        let scale = bp.version.map_or(DirectionScale::Sixteen, |v| {
            GameVersion::unpack(v).direction_scale()
        });
        let entities = bp
            .entities
            .iter()
            .map(|e| convert_entity(e, scale))
            .collect::<Result<_, _>>()?;
        Ok(Self { entities })
    }

    pub fn from_root(root: &raw::Root) -> Result<Vec<Self>, ModelError> {
        root.blueprints().map(Self::from_raw).collect()
    }

    /// Exports the model to a 2.0 blueprint. Entity numbers are assigned from 1 in `entities` order.
    pub fn to_blueprint(&self) -> raw::Blueprint {
        raw::Blueprint {
            entities: self
                .entities
                .iter()
                .enumerate()
                .map(|(i, e)| export_entity(i as u64 + 1, e))
                .collect(),
            tiles: Vec::new(),
            wires: Vec::new(),
            label: None,
            description: None,
            version: Some(EXPORT_VERSION),
            extra: serde_json::Map::new(),
        }
    }
}

/// Factorio 2.0.0.0 packed as 4×u16.
const EXPORT_VERSION: u64 = 2 << 48;

/// Inventory index for modules of crafting machines in the 2.0 format.
const MODULE_INVENTORY: u8 = 4;

fn export_entity(number: u64, e: &Entity) -> raw::Entity {
    let quality_of = |q: &Quality| (q.as_str() != "normal").then(|| q.as_str().to_owned());
    let items = (!e.modules.is_empty()).then(|| {
        let mut slot = 0u32;
        let requests = e.modules.iter().map(|stack| {
            let slots: Vec<_> = (0..stack.count)
                .map(|_| {
                    slot += 1;
                    serde_json::json!({ "inventory": MODULE_INVENTORY, "stack": slot - 1, "count": 1 })
                })
                .collect();
            serde_json::json!({
                "id": { "name": &*stack.item, "quality": stack.quality.as_str() },
                "items": { "in_inventory": slots },
            })
        });
        serde_json::Value::Array(requests.collect())
    });
    raw::Entity {
        entity_number: number,
        name: e.name.to_string(),
        position: raw::Position {
            x: e.pos.x(),
            y: e.pos.y(),
        },
        direction: (e.direction != Direction::NORTH).then_some(e.direction.index() as u8),
        mirror: e.mirror.then_some(true),
        recipe: e.recipe.as_ref().map(|r| r.name.to_string()),
        recipe_quality: e.recipe.as_ref().and_then(|r| quality_of(&r.quality)),
        quality: quality_of(&e.quality),
        items,
        extra: e
            .io
            .map(|io| serde_json::Map::from_iter([("type".to_owned(), io.as_str().into())]))
            .unwrap_or_default(),
    }
}

fn convert_entity(e: &raw::Entity, scale: DirectionScale) -> Result<Entity, ModelError> {
    let pos = CenterPos::from_f64(e.position.x, e.position.y).ok_or(ModelError::OffGrid {
        entity: e.entity_number,
        x: e.position.x,
        y: e.position.y,
    })?;
    let direction = match e.direction {
        None => Direction::NORTH,
        Some(d) => Direction::from_blueprint(d, scale).ok_or(ModelError::InvalidDirection {
            entity: e.entity_number,
            direction: d,
        })?,
    };
    let modules = match &e.items {
        None => Vec::new(),
        Some(v) => items::parse(v).map_err(|reason| ModelError::InvalidItems {
            entity: e.entity_number,
            reason,
        })?,
    };
    let quality_of = |q: &Option<String>| q.as_deref().map_or_else(Quality::normal, Quality::new);
    Ok(Entity {
        name: EntityName::new(&e.name),
        pos,
        direction,
        mirror: e.mirror.unwrap_or(false),
        quality: quality_of(&e.quality),
        recipe: e.recipe.as_deref().map(|name| Recipe {
            name: RecipeName::new(name),
            quality: quality_of(&e.recipe_quality),
        }),
        modules,
        io: e
            .extra
            .get("type")
            .and_then(serde_json::Value::as_str)
            .and_then(IoType::parse),
    })
}

#[cfg(test)]
mod tests;
