use serde::Deserialize;

use crate::effects::Effects;
use crate::error::{Error, Result};
use crate::raw::{Prototype, RawData};
use crate::serde_util::lenient_vec;

#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    pub kind: String,
    pub name: String,
    pub crafting_speed: f64,
    pub crafting_categories: Vec<String>,
    pub module_slots: u32,
    pub energy_usage_w: f64,
    pub allowed_effects: Vec<String>,
    pub base_effect: Effects,
}

#[derive(Deserialize)]
struct MachineDef {
    name: String,
    crafting_speed: f64,
    #[serde(default, deserialize_with = "lenient_vec")]
    crafting_categories: Vec<String>,
    #[serde(default)]
    module_slots: u32,
    energy_usage: Option<String>,
    #[serde(default, deserialize_with = "lenient_vec")]
    allowed_effects: Vec<String>,
    #[serde(default)]
    effect_receiver: Option<EffectReceiver>,
}

#[derive(Deserialize)]
struct EffectReceiver {
    #[serde(default)]
    base_effect: Effects,
}

/// Parses a Factorio energy string (`"150kW"`) into watts. Hand-rolled: the format is a tiny
/// SI-prefix subset, and no maintained crate parses Factorio's unit strings.
pub fn parse_power(text: &str) -> Result<f64> {
    let invalid = || Error::InvalidEnergy(text.to_owned());
    let body = text.trim().strip_suffix('W').ok_or_else(invalid)?;
    let (number, multiplier) = match body.chars().last() {
        Some(c) if c.is_ascii_alphabetic() => {
            let multiplier = match c {
                'k' => 1e3,
                'M' => 1e6,
                'G' => 1e9,
                'T' => 1e12,
                'P' => 1e15,
                _ => return Err(invalid()),
            };
            (&body[..body.len() - 1], multiplier)
        }
        _ => (body, 1.0),
    };
    number
        .parse::<f64>()
        .map(|n| n * multiplier)
        .map_err(|_| invalid())
}

fn convert(proto: Prototype<'_>) -> Result<Machine> {
    let def: MachineDef = proto.parse()?;
    Ok(Machine {
        kind: proto.kind.to_owned(),
        name: def.name,
        crafting_speed: def.crafting_speed,
        crafting_categories: def.crafting_categories,
        module_slots: def.module_slots,
        energy_usage_w: def
            .energy_usage
            .as_deref()
            .map(parse_power)
            .transpose()?
            .unwrap_or(0.0),
        allowed_effects: def.allowed_effects,
        base_effect: def
            .effect_receiver
            .map(|r| r.base_effect)
            .unwrap_or_default(),
    })
}

impl RawData {
    pub fn machine(&self, name: &str) -> Result<Option<Machine>> {
        self.machines.get_or_compute(name, || {
            self.all()
                .find(|p| p.name == name && p.has("crafting_speed"))
                .map(convert)
                .transpose()
        })
    }

    pub fn machines(&self) -> impl Iterator<Item = Result<Machine>> + '_ {
        self.all().filter(|p| p.has("crafting_speed")).map(convert)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_power() {
        assert_eq!(parse_power("150kW").unwrap(), 150_000.0);
        assert_eq!(parse_power("2.5MW").unwrap(), 2_500_000.0);
        assert_eq!(parse_power("90W").unwrap(), 90.0);
        assert!(parse_power("90J").is_err());
        assert!(parse_power("fastW").is_err());
        assert!(parse_power("").is_err());
    }
}
