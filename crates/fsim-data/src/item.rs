use serde::Deserialize;

use crate::error::Result;
use crate::raw::{Prototype, RawData};

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub kind: String,
    pub name: String,
    pub stack_size: u32,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Fluid {
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Deserialize)]
struct ItemDef {
    name: String,
    stack_size: u32,
    #[serde(default)]
    icon: Option<String>,
}

fn convert(proto: Prototype<'_>) -> Result<Item> {
    let def: ItemDef = proto.parse()?;
    Ok(Item {
        kind: proto.kind.to_owned(),
        name: def.name,
        stack_size: def.stack_size,
        icon: def.icon,
    })
}

impl RawData {
    pub fn item(&self, name: &str) -> Result<Option<Item>> {
        self.all()
            .find(|p| p.name == name && p.has("stack_size"))
            .map(convert)
            .transpose()
    }

    pub fn items(&self) -> impl Iterator<Item = Result<Item>> + '_ {
        self.all().filter(|p| p.has("stack_size")).map(convert)
    }

    pub fn fluid(&self, name: &str) -> Result<Option<Fluid>> {
        self.get("fluid", name)
            .map(|value| {
                Prototype {
                    kind: "fluid",
                    name,
                    value,
                }
                .parse()
            })
            .transpose()
    }

    pub fn fluids(&self) -> impl Iterator<Item = Result<Fluid>> + '_ {
        self.of_kind("fluid").map(|(name, value)| {
            Prototype {
                kind: "fluid",
                name,
                value,
            }
            .parse()
        })
    }
}
