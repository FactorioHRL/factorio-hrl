use serde::Deserialize;
use serde_json::Value;

use super::names::{ItemName, Quality};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStack {
    pub item: ItemName,
    pub quality: Quality,
    pub count: u32,
}

#[derive(Deserialize)]
struct Request2 {
    id: Id2,
    #[serde(default)]
    items: Items2,
}

#[derive(Deserialize)]
struct Id2 {
    name: String,
    #[serde(default)]
    quality: Option<String>,
}

#[derive(Deserialize, Default)]
struct Items2 {
    #[serde(default)]
    in_inventory: Vec<Slot2>,
}

#[derive(Deserialize)]
struct Slot2 {
    #[serde(default = "one")]
    count: u32,
}

fn one() -> u32 {
    1
}

pub(super) fn parse(value: &Value) -> Result<Vec<ItemStack>, String> {
    let mut stacks = parse_raw(value)?;
    stacks.sort_by(|a, b| (&a.item, &a.quality).cmp(&(&b.item, &b.quality)));
    stacks.dedup_by(|dup, kept| {
        let same = dup.item == kept.item && dup.quality == kept.quality;
        if same {
            kept.count += dup.count;
        }
        same
    });
    Ok(stacks)
}

fn parse_raw(value: &Value) -> Result<Vec<ItemStack>, String> {
    match value {
        Value::Array(_) => {
            let requests: Vec<Request2> =
                serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            Ok(requests.into_iter().flat_map(expand_2_0).collect())
        }
        Value::Object(map) => map
            .iter()
            .map(|(name, count)| {
                let count = count
                    .as_u64()
                    .and_then(|c| u32::try_from(c).ok())
                    .ok_or_else(|| format!("invalid count for '{name}'"))?;
                Ok(ItemStack {
                    item: ItemName::new(name),
                    quality: Quality::normal(),
                    count,
                })
            })
            .collect(),
        _ => Err("expected array or object".into()),
    }
}

fn expand_2_0(request: Request2) -> Vec<ItemStack> {
    let item = ItemName::new(&request.id.name);
    let quality = request
        .id
        .quality
        .as_deref()
        .map_or_else(Quality::normal, Quality::new);
    request
        .items
        .in_inventory
        .into_iter()
        .map(|slot| ItemStack {
            item: item.clone(),
            quality: quality.clone(),
            count: slot.count,
        })
        .collect()
}
