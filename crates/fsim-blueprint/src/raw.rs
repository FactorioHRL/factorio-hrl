use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Root {
    Blueprint(Blueprint),
    BlueprintBook(BlueprintBook),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Blueprint {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<Entity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiles: Vec<Tile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wires: Vec<[u64; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlueprintBook {
    #[serde(default)]
    pub blueprints: Vec<BookEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookEntry {
    pub index: u32,
    #[serde(flatten)]
    pub content: Root,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub name: String,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub entity_number: u64,
    pub name: String,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Root {
    pub fn blueprints(&self) -> Box<dyn Iterator<Item = &Blueprint> + '_> {
        match self {
            Root::Blueprint(bp) => Box::new(std::iter::once(bp)),
            Root::BlueprintBook(book) => {
                Box::new(book.blueprints.iter().flat_map(|e| e.content.blueprints()))
            }
        }
    }
}
