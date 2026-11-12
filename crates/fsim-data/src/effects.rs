use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
pub struct Effects {
    #[serde(default)]
    pub speed: f64,
    #[serde(default)]
    pub productivity: f64,
    #[serde(default)]
    pub consumption: f64,
    #[serde(default)]
    pub pollution: f64,
    #[serde(default)]
    pub quality: f64,
}
