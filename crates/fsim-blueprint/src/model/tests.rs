use serde_json::json;

use super::*;

const SAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/sample-blueprints/example_1"
);

fn raw_blueprint(value: serde_json::Value) -> raw::Blueprint {
    serde_json::from_value(value).unwrap()
}

#[test]
fn sample_converts() {
    let root = crate::load(SAMPLE).unwrap();
    let designs = Design::from_root(&root).unwrap();
    assert_eq!(designs.len(), 1);
    let design = &designs[0];
    assert_eq!(design.entities.len(), 236);

    let plants: Vec<_> = design
        .entities
        .iter()
        .filter(|e| &*e.name == "chemical-plant")
        .collect();
    assert_eq!(plants.len(), 9);
    assert!(plants.iter().all(|e| e.recipe.is_some()));

    let with_modules = plants.iter().find(|e| !e.modules.is_empty()).unwrap();
    assert_eq!(
        with_modules.modules,
        vec![ItemStack {
            item: ItemName::new("efficiency-module-2"),
            quality: Quality::normal(),
            count: 2,
        }]
    );
}

#[test]
fn missing_direction_is_north() {
    let design = Design::from_raw(&raw_blueprint(json!({
        "entities": [{"entity_number": 1, "name": "pipe", "position": {"x": 0.5, "y": -1.5}}]
    })))
    .unwrap();
    let e = &design.entities[0];
    assert_eq!(e.direction, Direction::NORTH);
    assert_eq!(e.pos, CenterPos::new(1, -3));
    assert_eq!((e.pos.x(), e.pos.y()), (0.5, -1.5));
}

#[test]
fn old_format_directions_and_items() {
    let design = Design::from_raw(&raw_blueprint(json!({
        "version": 281474979332096u64,
        "entities": [{
            "entity_number": 1, "name": "assembling-machine-2",
            "position": {"x": 0.5, "y": 0.5}, "direction": 2,
            "items": {"productivity-module": 2}
        }]
    })))
    .unwrap();
    let e = &design.entities[0];
    assert_eq!(e.direction, Direction::EAST);
    assert_eq!(e.modules.len(), 1);
    assert_eq!(e.modules[0].count, 2);
}

#[test]
fn module_slots_are_aggregated() {
    let slot = |stack| json!({"inventory": 4, "stack": stack});
    let design = Design::from_raw(&raw_blueprint(json!({
        "entities": [{
            "entity_number": 1, "name": "beacon", "position": {"x": 0.5, "y": 0.5},
            "items": [
                {"id": {"name": "speed-module-3"}, "items": {"in_inventory": [slot(0), slot(1)]}},
                {"id": {"name": "speed-module-3", "quality": "rare"}, "items": {"in_inventory": [slot(2)]}}
            ]
        }]
    })))
    .unwrap();
    let modules = &design.entities[0].modules;
    assert_eq!(modules.len(), 2);
    assert_eq!(
        (modules[0].quality.as_str(), modules[0].count),
        ("normal", 2)
    );
    assert_eq!((modules[1].quality.as_str(), modules[1].count), ("rare", 1));
}

#[test]
fn recipe_and_quality() {
    let design = Design::from_raw(&raw_blueprint(json!({
        "entities": [{
            "entity_number": 1, "name": "chemical-plant", "position": {"x": 1.5, "y": 1.5},
            "recipe": "plastic-bar", "recipe_quality": "uncommon", "quality": "rare", "mirror": true
        }]
    })))
    .unwrap();
    let e = &design.entities[0];
    let recipe = e.recipe.as_ref().unwrap();
    assert_eq!(&*recipe.name, "plastic-bar");
    assert_eq!(recipe.quality.as_str(), "uncommon");
    assert_eq!(e.quality.as_str(), "rare");
    assert!(e.mirror);
}

#[test]
fn rejects_bad_input() {
    let entity = |extra: serde_json::Value| {
        let mut e = json!({"entity_number": 1, "name": "pipe", "position": {"x": 0.5, "y": 0.5}});
        e.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        raw_blueprint(json!({ "entities": [e] }))
    };

    let off_grid = json!({"position": {"x": 0.25, "y": 0.5}});
    assert!(matches!(
        Design::from_raw(&entity(off_grid)),
        Err(ModelError::OffGrid { entity: 1, .. })
    ));
    assert_eq!(
        Design::from_raw(&entity(json!({"direction": 16}))),
        Err(ModelError::InvalidDirection {
            entity: 1,
            direction: 16
        })
    );
    assert!(matches!(
        Design::from_raw(&entity(json!({"items": 5}))),
        Err(ModelError::InvalidItems { entity: 1, .. })
    ));
}

#[test]
fn export_roundtrips_through_the_model() {
    let root = crate::load(SAMPLE).unwrap();
    let design = Design::from_root(&root).unwrap().remove(0);
    let exported = design.to_blueprint();
    assert_eq!(exported.entities.len(), design.entities.len());
    assert_eq!(exported.entities[0].entity_number, 1);

    let encoded = crate::encode(&raw::Root::Blueprint(exported)).unwrap();
    let again = Design::from_root(&crate::decode(&encoded).unwrap())
        .unwrap()
        .remove(0);
    assert_eq!(again, design);
}

#[test]
fn underground_type_is_read_and_exported() {
    let bp = raw_blueprint(json!({"entities": [
        {"entity_number": 1, "name": "underground-belt", "position": {"x": 0.5, "y": 0.5},
         "direction": 4, "type": "input"},
        {"entity_number": 2, "name": "underground-belt", "position": {"x": 3.5, "y": 0.5},
         "direction": 4, "type": "output"},
        {"entity_number": 3, "name": "transport-belt", "position": {"x": 5.5, "y": 0.5}}
    ]}));
    let design = Design::from_raw(&bp).unwrap();
    let io: Vec<_> = design.entities.iter().map(|e| e.io).collect();
    assert_eq!(io, [Some(IoType::Input), Some(IoType::Output), None]);

    let exported = design.to_blueprint();
    assert_eq!(exported.entities[0].extra["type"], "input");
    assert_eq!(exported.entities[1].extra["type"], "output");
    assert!(exported.entities[2].extra.is_empty());
    assert_eq!(Design::from_raw(&exported).unwrap(), design);
}

const RED_SCIENCE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/sample-blueprints/automation_science_red"
);

#[test]
fn red_science_sample_converts_with_1_1_directions_and_underground_types() {
    let root = crate::load(RED_SCIENCE).unwrap();
    let design = Design::from_root(&root).unwrap().remove(0);
    assert_eq!(design.entities.len(), 161);

    let count = |name: &str| design.entities.iter().filter(|e| &*e.name == name).count();
    assert_eq!(count("assembling-machine-1"), 8);
    assert_eq!(count("splitter"), 3);

    let recipes: std::collections::BTreeSet<_> = design
        .entities
        .iter()
        .filter_map(|e| e.recipe.as_ref().map(|r| r.name.to_string()))
        .collect();
    assert_eq!(
        recipes.into_iter().collect::<Vec<_>>(),
        ["automation-science-pack", "iron-gear-wheel"]
    );

    let undergrounds: Vec<_> = design
        .entities
        .iter()
        .filter(|e| &*e.name == "underground-belt")
        .collect();
    assert_eq!(undergrounds.len(), 8);
    assert!(undergrounds.iter().all(|e| e.io.is_some()));
    // blueprint 1.1: directions 0..7 are multiplied by 2, so all of them are even
    assert!(design.entities.iter().all(|e| e.direction.index() % 2 == 0));
}

const BEACON_ASSEMBLERS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/sample-blueprints/beacon_assemblers"
);

#[test]
fn beacon_sample_converts() {
    let root = crate::load(BEACON_ASSEMBLERS).unwrap();
    let design = Design::from_root(&root).unwrap().remove(0);
    assert_eq!(design.entities.len(), 209);

    let count = |name: &str| design.entities.iter().filter(|e| &*e.name == name).count();
    assert_eq!(count("beacon"), 26);
    assert_eq!(count("assembling-machine-3"), 12);
}
