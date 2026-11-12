use fsim_data::RawData;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Icon {
    pub path: String,
    pub size: u32,
}

const DEFAULT_ICON_SIZE: u32 = 64;

fn from_prototype(value: &Value) -> Option<Icon> {
    let size = |v: &Value| {
        v.get("icon_size")
            .and_then(Value::as_u64)
            .and_then(|s| u32::try_from(s).ok())
    };
    if let Some(path) = value.get("icon").and_then(Value::as_str) {
        return Some(Icon {
            path: path.to_owned(),
            size: size(value).unwrap_or(DEFAULT_ICON_SIZE),
        });
    }
    let layer = value.get("icons")?.as_array()?.first()?;
    Some(Icon {
        path: layer.get("icon")?.as_str()?.to_owned(),
        size: size(layer)
            .or_else(|| size(value))
            .unwrap_or(DEFAULT_ICON_SIZE),
    })
}

pub(crate) fn product_icon(data: &RawData, name: &str) -> Option<Icon> {
    if let Some(fluid) = data.get("fluid", name) {
        return from_prototype(fluid);
    }
    data.all()
        .find(|p| p.name == name && p.has("stack_size"))
        .and_then(|p| from_prototype(p.value))
}

pub(crate) fn recipe_icon(data: &RawData, recipe: &str) -> Option<Icon> {
    let value = data.get("recipe", recipe)?;
    if let Some(icon) = from_prototype(value) {
        return Some(icon);
    }
    let product = value
        .get("main_product")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .or_else(|| {
            let results = value.get("results")?.as_array()?;
            match results.as_slice() {
                [only] => only.get("name")?.as_str(),
                _ => None,
            }
        })?;
    product_icon(data, product)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> RawData {
        RawData::from_slice(
            br#"{
            "recipe": {
                "own": {"icon": "__base__/own.png", "icon_size": 32, "results": []},
                "plain": {"results": [{"name": "plate", "amount": 1}]},
                "main": {"main_product": "gas", "results": [{"name": "plate"}, {"name": "gas"}]},
                "many": {"results": [{"name": "plate"}, {"name": "gas"}]}
            },
            "item": {"plate": {"name": "plate", "stack_size": 100, "icon": "__base__/plate.png"}},
            "fluid": {"gas": {"name": "gas", "icons": [{"icon": "__base__/gas.png", "icon_size": 64}]}}
        }"#,
        )
        .unwrap()
    }

    #[test]
    fn recipe_own_icon_wins() {
        let icon = recipe_icon(&data(), "own").unwrap();
        assert_eq!((icon.path.as_str(), icon.size), ("__base__/own.png", 32));
    }

    #[test]
    fn falls_back_to_the_only_product() {
        let icon = recipe_icon(&data(), "plain").unwrap();
        assert_eq!((icon.path.as_str(), icon.size), ("__base__/plate.png", 64));
    }

    #[test]
    fn main_product_may_be_a_fluid_with_layered_icons() {
        assert_eq!(
            recipe_icon(&data(), "main").unwrap().path,
            "__base__/gas.png"
        );
    }

    #[test]
    fn ambiguous_or_missing_recipes_have_no_icon() {
        assert!(recipe_icon(&data(), "many").is_none());
        assert!(recipe_icon(&data(), "nope").is_none());
    }
}
