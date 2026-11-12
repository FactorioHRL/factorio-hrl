use fsim_data::{Effects, EntityGeometry, ItemKind, RawData};

fn excerpt() -> RawData {
    RawData::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/data-excerpt.json"
    ))
    .unwrap()
}

#[test]
fn recipe_with_defaults() {
    let gear = excerpt().recipe("iron-gear-wheel").unwrap().unwrap();
    assert_eq!(gear.categories, ["crafting"]);
    assert_eq!(gear.energy_required, 0.5);
    assert_eq!(gear.ingredients.len(), 1);
    assert_eq!(gear.ingredients[0].name, "iron-plate");
    assert_eq!(gear.ingredients[0].amount, 2.0);
    assert_eq!(gear.results[0].amount_min, 1.0);
    assert_eq!(gear.results[0].probability, 1.0);
    assert!(gear.allow_productivity);
}

#[test]
fn recipe_with_fluid_and_category() {
    let plastic = excerpt().recipe("plastic-bar").unwrap().unwrap();
    assert_eq!(plastic.categories, ["chemistry-or-cryogenics"]);
    assert_eq!(plastic.energy_required, 1.0);
    assert!(plastic
        .ingredients
        .iter()
        .any(|i| i.kind == ItemKind::Fluid));
    assert_eq!(plastic.results[0].amount_max, 2.0);
}

#[test]
fn recipe_with_probability_and_ignored_productivity() {
    let data = excerpt();
    let uranium = data.recipe("uranium-processing").unwrap().unwrap();
    assert_eq!(uranium.results[0].probability, 0.007000000000000001);
    assert_eq!(uranium.results[1].probability, 0.993);

    let kovarex = data.recipe("kovarex-enrichment-process").unwrap().unwrap();
    assert_eq!(kovarex.results[0].ignored_by_productivity, 40.0);
    assert!(!kovarex.allow_quality);
}

#[test]
fn fluidbox_index_is_kept() {
    let data = RawData::from_slice(
        br#"{"recipe": {"oil": {"type": "recipe", "name": "oil",
            "ingredients": [{"type": "fluid", "name": "crude-oil", "amount": 100, "fluidbox_index": 2}],
            "results": [{"type": "fluid", "name": "petroleum-gas", "amount": 45, "fluidbox_index": 3},
                        {"type": "item", "name": "sulfur", "amount": 1}]}}}"#,
    )
    .unwrap();
    let oil = data.recipe("oil").unwrap().unwrap();
    assert_eq!(oil.ingredients[0].fluidbox_index, Some(2));
    assert_eq!(oil.results[0].fluidbox_index, Some(3));
    assert_eq!(oil.results[1].fluidbox_index, None);
}

#[test]
fn empty_lua_tables_are_accepted() {
    let data = excerpt();
    let unknown = data.recipe("recipe-unknown").unwrap().unwrap();
    assert!(unknown.ingredients.is_empty());
    assert!(unknown.results.is_empty());
    let parameter = data.recipe("parameter-0").unwrap().unwrap();
    assert!(parameter.results.is_empty());
}

#[test]
fn missing_recipe_is_none() {
    assert!(excerpt().recipe("nope").unwrap().is_none());
}

#[test]
fn all_recipes_load() {
    let data = excerpt();
    let count = data.recipes().collect::<Result<Vec<_>, _>>().unwrap().len();
    assert_eq!(count, data.of_kind("recipe").count());
}

#[test]
fn machines() {
    let data = excerpt();

    let plant = data.machine("chemical-plant").unwrap().unwrap();
    assert_eq!(plant.crafting_speed, 1.0);
    assert_eq!(plant.module_slots, 3);
    assert_eq!(plant.energy_usage_w, 210_000.0);
    assert!(plant.crafting_categories.contains(&"chemistry".to_owned()));
    assert!(plant.allowed_effects.contains(&"productivity".to_owned()));
    assert_eq!(plant.base_effect, Effects::default());

    let biochamber = data.machine("biochamber").unwrap().unwrap();
    assert_eq!(biochamber.base_effect.productivity, 0.5);

    let furnace = data.machine("stone-furnace").unwrap().unwrap();
    assert_eq!(furnace.kind, "furnace");
    assert_eq!(furnace.module_slots, 0);

    let spawner = data.machine("captive-biter-spawner").unwrap().unwrap();
    assert!(spawner.allowed_effects.is_empty());

    assert!(data.machine("transport-belt").unwrap().is_none());
    assert_eq!(data.machines().count(), 6);
}

#[test]
fn modules() {
    let module = excerpt().module("efficiency-module-2").unwrap().unwrap();
    assert_eq!(module.category, "efficiency");
    assert_eq!(module.tier, 2);
    assert_eq!(module.effect.consumption, -0.4);
    assert_eq!(module.effect.speed, 0.0);
}

#[test]
fn items_and_fluids() {
    let data = excerpt();
    let plate = data.item("iron-plate").unwrap().unwrap();
    assert_eq!(plate.stack_size, 100);
    assert!(plate.icon.unwrap().starts_with("__base__/"));
    assert!(data.fluid("petroleum-gas").unwrap().is_some());
    assert!(data.item("petroleum-gas").unwrap().is_none());
}

#[test]
fn geometry_from_collision_box() {
    let data = excerpt();
    let size = |name: &str| data.entity_geometry(name).unwrap().unwrap().geometry;
    assert_eq!(
        size("chemical-plant"),
        EntityGeometry {
            width: 3,
            height: 3
        }
    );
    assert_eq!(
        size("oil-refinery"),
        EntityGeometry {
            width: 5,
            height: 5
        }
    );
    assert_eq!(
        size("stone-furnace"),
        EntityGeometry {
            width: 2,
            height: 2
        }
    );
    assert_eq!(
        size("transport-belt"),
        EntityGeometry {
            width: 1,
            height: 1
        }
    );
    assert_eq!(
        size("pipe"),
        EntityGeometry {
            width: 1,
            height: 1
        }
    );
    assert_eq!(
        size("beacon"),
        EntityGeometry {
            width: 3,
            height: 3
        }
    );
}

#[test]
fn geometry_rotation() {
    let g = EntityGeometry {
        width: 2,
        height: 3,
    };
    assert_eq!(g.oriented(0), g);
    assert_eq!(
        g.oriented(1),
        EntityGeometry {
            width: 3,
            height: 2
        }
    );
    assert_eq!(g.oriented(2), g);
}

#[test]
fn unknown_fields_and_bad_input() {
    let data = RawData::from_slice(
        br#"{"recipe": {"x": {"type": "recipe", "name": "x", "wat": 1,
            "results": [{"name": "a", "amount": 1}]},
            "bad": {"type": "recipe", "name": "bad", "results": [{"name": "a"}]}}}"#,
    )
    .unwrap();
    assert!(data.recipe("x").unwrap().is_some());
    assert!(data.recipe("bad").is_err());
    assert!(RawData::from_slice(b"not json").is_err());
    assert!(RawData::load("/nonexistent/dump.json").is_err());
}

#[test]
fn full_vanilla_dump_loads() {
    let Some(path) = std::env::var_os("FSIM_DUMP") else {
        eprintln!("FSIM_DUMP not set, skipping");
        return;
    };
    let started = std::time::Instant::now();
    let data = RawData::load(path).unwrap();
    eprintln!("loaded in {:?}", started.elapsed());

    let recipes = data.recipes().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(recipes.len(), data.of_kind("recipe").count());
    data.machines().collect::<Result<Vec<_>, _>>().unwrap();
    data.modules().collect::<Result<Vec<_>, _>>().unwrap();
    data.items().collect::<Result<Vec<_>, _>>().unwrap();
    data.fluids().collect::<Result<Vec<_>, _>>().unwrap();
    data.entity_geometries()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let size = |name: &str| {
        let g = data.entity_geometry(name).unwrap().unwrap().geometry;
        (g.width, g.height)
    };
    assert_eq!(size("assembling-machine-3"), (3, 3));
    assert_eq!(size("electric-furnace"), (3, 3));
    assert_eq!(size("rocket-silo"), (9, 9));
    assert_eq!(size("nuclear-reactor"), (5, 5));
    assert_eq!(size("foundry"), (5, 5));
    assert_eq!(size("small-electric-pole"), (1, 1));
    assert_eq!(size("big-electric-pole"), (2, 2));
    assert_eq!(size("substation"), (2, 2));
    assert_eq!(size("fast-inserter"), (1, 1));
    assert_eq!(size("splitter"), (2, 1));
    assert_eq!(size("storage-tank"), (3, 3));
}

#[test]
fn categories_list_from_2_1_dumps() {
    let data = RawData::from_slice(
        br#"{"recipe": {
            "plastic-bar": {"name": "plastic-bar", "categories": ["chemistry", "cryogenics"]},
            "old": {"name": "old", "category": "smelting", "additional_categories": ["x"]}
        }}"#,
    )
    .unwrap();
    let plastic = data.recipe("plastic-bar").unwrap().unwrap();
    assert_eq!(plastic.categories, ["chemistry", "cryogenics"]);
    let old = data.recipe("old").unwrap().unwrap();
    assert_eq!(old.categories, ["smelting", "x"]);
}
