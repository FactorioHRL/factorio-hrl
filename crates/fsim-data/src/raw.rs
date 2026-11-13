use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::error::{Error, Result};
use crate::geometry::EntityInfo;
use crate::machine::Machine;
use crate::memo::Memo;
use crate::recipe::Recipe;

#[derive(Debug, Clone, Default)]
pub struct RawData {
    prototypes: BTreeMap<String, BTreeMap<String, Value>>,
    // Parsed views, remembered per name: the simulator asks for the same few prototypes on every
    // step, and finding and parsing one means scanning the dump.
    pub(crate) recipes: Memo<Recipe>,
    pub(crate) machines: Memo<Machine>,
    pub(crate) geometries: Memo<EntityInfo>,
}

impl RawData {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_slice(&bytes)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        Ok(Self {
            prototypes: serde_json::from_slice(bytes)?,
            ..Self::default()
        })
    }

    pub fn get(&self, kind: &str, name: &str) -> Option<&Value> {
        self.prototypes.get(kind)?.get(name)
    }

    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.prototypes.keys().map(String::as_str)
    }

    pub fn of_kind<'a>(&'a self, kind: &str) -> impl Iterator<Item = (&'a str, &'a Value)> {
        self.prototypes
            .get(kind)
            .into_iter()
            .flatten()
            .map(|(name, value)| (name.as_str(), value))
    }

    pub fn all(&self) -> impl Iterator<Item = Prototype<'_>> {
        self.prototypes.iter().flat_map(|(kind, by_name)| {
            by_name
                .iter()
                .map(move |(name, value)| Prototype { kind, name, value })
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Prototype<'a> {
    pub kind: &'a str,
    pub name: &'a str,
    pub value: &'a Value,
}

impl Prototype<'_> {
    pub fn has(&self, field: &str) -> bool {
        self.value.get(field).is_some()
    }

    pub fn parse_into<D, T>(&self) -> Result<T>
    where
        D: serde::de::DeserializeOwned,
        T: TryFrom<D, Error = String>,
    {
        let def: D = self.parse()?;
        T::try_from(def).map_err(|msg| Error::Prototype {
            kind: self.kind.to_owned(),
            name: self.name.to_owned(),
            source: serde::de::Error::custom(msg),
        })
    }

    pub fn parse<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        T::deserialize(self.value).map_err(|source| Error::Prototype {
            kind: self.kind.to_owned(),
            name: self.name.to_owned(),
            source,
        })
    }
}
