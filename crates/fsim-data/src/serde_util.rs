use serde::de::{DeserializeOwned, Error};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Accepts a JSON array, `null` or an empty object (how Lua dumps an empty table) as a `Vec`.
/// Hand-rolled: `serde_with` covers `null` but not the empty-object case.
pub fn lenient_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    match Value::deserialize(deserializer)? {
        Value::Null => Ok(Vec::new()),
        Value::Object(map) if map.is_empty() => Ok(Vec::new()),
        value @ Value::Array(_) => serde_json::from_value(value).map_err(D::Error::custom),
        _ => Err(D::Error::custom("expected array")),
    }
}
