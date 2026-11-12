use serde::Deserialize;

use crate::effects::Effects;
use crate::error::Result;
use crate::raw::{Prototype, RawData};

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Module {
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub tier: u32,
    #[serde(default)]
    pub effect: Effects,
}

impl RawData {
    pub fn module(&self, name: &str) -> Result<Option<Module>> {
        self.get("module", name)
            .map(|value| {
                Prototype {
                    kind: "module",
                    name,
                    value,
                }
                .parse()
            })
            .transpose()
    }

    pub fn modules(&self) -> impl Iterator<Item = Result<Module>> + '_ {
        self.of_kind("module").map(|(name, value)| {
            Prototype {
                kind: "module",
                name,
                value,
            }
            .parse()
        })
    }
}
