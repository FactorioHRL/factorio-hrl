use serde::Deserialize;

use crate::error::Result;
use crate::raw::{Prototype, RawData};
use crate::serde_util::lenient_vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    #[default]
    Item,
    Fluid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ingredient {
    pub kind: ItemKind,
    pub name: String,
    pub amount: f64,
    pub fluidbox_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Product {
    pub kind: ItemKind,
    pub name: String,
    pub amount_min: f64,
    pub amount_max: f64,
    pub probability: f64,
    pub ignored_by_productivity: f64,
    pub extra_count_fraction: f64,
    pub fluidbox_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub name: String,
    pub categories: Vec<String>,
    pub energy_required: f64,
    pub ingredients: Vec<Ingredient>,
    pub results: Vec<Product>,
    pub allow_productivity: bool,
    pub allow_quality: bool,
}

#[derive(Deserialize)]
struct IngredientDef {
    #[serde(default, rename = "type")]
    kind: ItemKind,
    name: String,
    amount: f64,
    fluidbox_index: Option<u32>,
}

#[derive(Deserialize)]
struct ProductDef {
    #[serde(default, rename = "type")]
    kind: ItemKind,
    name: String,
    amount: Option<f64>,
    amount_min: Option<f64>,
    amount_max: Option<f64>,
    #[serde(default = "one")]
    probability: f64,
    #[serde(default)]
    ignored_by_productivity: Option<f64>,
    #[serde(default)]
    extra_count_fraction: f64,
    fluidbox_index: Option<u32>,
}

#[derive(Deserialize)]
struct RecipeDef {
    name: String,
    category: Option<String>,
    /// The 2.1 dump stores categories as a single list instead of `category` + `additional_categories`.
    #[serde(default, deserialize_with = "lenient_vec")]
    categories: Vec<String>,
    #[serde(default, deserialize_with = "lenient_vec")]
    additional_categories: Vec<String>,
    #[serde(default = "default_energy")]
    energy_required: f64,
    #[serde(default, deserialize_with = "lenient_vec")]
    ingredients: Vec<IngredientDef>,
    #[serde(default, deserialize_with = "lenient_vec")]
    results: Vec<ProductDef>,
    #[serde(default)]
    allow_productivity: bool,
    #[serde(default = "yes")]
    allow_quality: bool,
}

fn one() -> f64 {
    1.0
}

fn yes() -> bool {
    true
}

fn default_energy() -> f64 {
    0.5
}

impl TryFrom<ProductDef> for Product {
    type Error = String;

    fn try_from(def: ProductDef) -> std::result::Result<Self, String> {
        let (amount_min, amount_max) = match (def.amount, def.amount_min, def.amount_max) {
            (Some(amount), _, _) => (amount, amount),
            (None, Some(min), Some(max)) => (min, max),
            _ => return Err(format!("product '{}' has no amount", def.name)),
        };
        Ok(Product {
            kind: def.kind,
            name: def.name,
            amount_min,
            amount_max,
            probability: def.probability,
            ignored_by_productivity: def.ignored_by_productivity.unwrap_or(0.0),
            extra_count_fraction: def.extra_count_fraction,
            fluidbox_index: def.fluidbox_index,
        })
    }
}

impl TryFrom<RecipeDef> for Recipe {
    type Error = String;

    fn try_from(def: RecipeDef) -> std::result::Result<Self, String> {
        let categories = if def.categories.is_empty() {
            let mut categories = vec![def.category.unwrap_or_else(|| "crafting".to_owned())];
            categories.extend(def.additional_categories);
            categories
        } else {
            def.categories
        };
        Ok(Recipe {
            name: def.name,
            categories,
            energy_required: def.energy_required,
            ingredients: def
                .ingredients
                .into_iter()
                .map(|i| Ingredient {
                    kind: i.kind,
                    name: i.name,
                    amount: i.amount,
                    fluidbox_index: i.fluidbox_index,
                })
                .collect(),
            results: def
                .results
                .into_iter()
                .map(Product::try_from)
                .collect::<std::result::Result<_, _>>()?,
            allow_productivity: def.allow_productivity,
            allow_quality: def.allow_quality,
        })
    }
}

impl RawData {
    pub fn recipe(&self, name: &str) -> Result<Option<Recipe>> {
        self.get("recipe", name)
            .map(|value| {
                Prototype {
                    kind: "recipe",
                    name,
                    value,
                }
                .parse_into::<RecipeDef, Recipe>()
            })
            .transpose()
    }

    pub fn recipes(&self) -> impl Iterator<Item = Result<Recipe>> + '_ {
        self.of_kind("recipe").map(|(name, value)| {
            Prototype {
                kind: "recipe",
                name,
                value,
            }
            .parse_into::<RecipeDef, Recipe>()
        })
    }
}
