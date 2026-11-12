use std::collections::HashSet;

use fsim_blueprint::model::Design;
use fsim_data::RawData;
use fsim_gen::legacy::{generate, Style};
use fsim_gen::{to_blueprint_string, Params};

fn load() -> Option<RawData> {
    let Some(dump) = std::env::var_os("FSIM_DUMP") else {
        eprintln!("FSIM_DUMP not set, skipping");
        return None;
    };
    Some(RawData::load(dump).unwrap())
}

fn footprint(data: &RawData, e: &fsim_blueprint::model::Entity) -> (i32, i32, i32, i32) {
    let g = data.entity_geometry(&e.name).unwrap().unwrap().geometry;
    let quarter = (((e.direction.index() + 2) / 4) % 4) as u8;
    let g = g.oriented(quarter);
    let (w, h) = (g.width as i32, g.height as i32);
    // x2 = 2 * lewy + w
    ((e.pos.x2 - w) / 2, (e.pos.y2 - h) / 2, w, h)
}

#[test]
fn legacy_designs_fill_exactly_the_area_without_overlaps() {
    let Some(data) = load() else { return };
    for style in Style::ALL {
        for seed in 0..40 {
            let design = generate(&data, style, Params::square(32, seed)).unwrap();
            let (mut min_x, mut min_y, mut max_x, mut max_y) =
                (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            let mut taken = HashSet::new();
            for e in &design.entities {
                let (x, y, w, h) = footprint(&data, e);
                assert_eq!(
                    (e.pos.x2 - w) % 2,
                    0,
                    "{style:?}/{seed}: {} off the tile grid",
                    e.name
                );
                for ty in y..y + h {
                    for tx in x..x + w {
                        assert!(
                            taken.insert((tx, ty)),
                            "{style:?}/{seed}: {} overlaps at ({tx},{ty})",
                            e.name
                        );
                    }
                }
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + w);
                max_y = max_y.max(y + h);
            }
            assert_eq!(
                (min_x, min_y, max_x, max_y),
                (0, 0, 32, 32),
                "{style:?}/{seed}: bounding box is not 32x32"
            );
        }
    }
}

#[test]
fn legacy_machines_get_recipes_and_generation_is_deterministic() {
    let Some(data) = load() else { return };
    for style in Style::ALL {
        let a = generate(&data, style, Params::square(32, 5)).unwrap();
        let b = generate(&data, style, Params::square(32, 5)).unwrap();
        let c = generate(&data, style, Params::square(32, 6)).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        for e in &a.entities {
            if e.name.starts_with("assembling-machine") || &*e.name == "chemical-plant" {
                let recipe = e.recipe.as_ref().expect("machine without recipe");
                assert!(data.recipe(&recipe.name).unwrap().is_some());
            }
        }
    }
}

#[test]
fn legacy_styles_contain_their_signature_entities() {
    let Some(data) = load() else { return };
    let count = |design: &Design, name: &str| {
        design
            .entities
            .iter()
            .filter(|e| e.name.contains(name))
            .count()
    };
    for seed in 0..10 {
        let asm = generate(&data, Style::Assemblers, Params::square(32, seed)).unwrap();
        let smelt = generate(&data, Style::Smelting, Params::square(32, seed)).unwrap();
        let chem = generate(&data, Style::Chemical, Params::square(32, seed)).unwrap();
        assert!(count(&asm, "assembling-machine") > 10 && count(&asm, "splitter") > 0);
        assert!(!underground_pairs(&asm).is_empty() || !underground_pairs(&chem).is_empty());
        assert!(count(&smelt, "furnace") > 20 && count(&smelt, "splitter") > 0);
        assert!(count(&chem, "oil-refinery") > 3 && count(&chem, "chemical-plant") > 5);
    }
}

#[test]
fn legacy_blueprint_string_decodes_back_to_the_same_design() {
    let Some(data) = load() else { return };
    for style in Style::ALL {
        let design = generate(&data, style, Params::square(32, 3)).unwrap();
        let text = to_blueprint_string(&design).unwrap();
        let root = fsim_blueprint::decode(&text).unwrap();
        assert_eq!(Design::from_root(&root).unwrap(), vec![design]);
    }
}

#[test]
fn legacy_too_small_areas_are_an_error() {
    let Some(data) = load() else { return };
    assert!(generate(&data, Style::Assemblers, Params::square(5, 1)).is_err());
}

fn underground_pairs(design: &Design) -> Vec<(usize, usize)> {
    use fsim_blueprint::model::IoType;
    let ends = |io| {
        design
            .entities
            .iter()
            .enumerate()
            .filter(move |(_, e)| e.name.contains("underground-belt") && e.io == Some(io))
            .map(|(i, _)| i)
            .collect::<Vec<_>>()
    };
    ends(IoType::Input)
        .into_iter()
        .zip(ends(IoType::Output))
        .collect()
}

#[test]
fn legacy_undergrounds_come_in_matching_pairs_within_range() {
    let Some(data) = load() else { return };
    let mut total = 0;
    for style in Style::ALL {
        for seed in 0..40 {
            let design = generate(&data, style, Params::square(32, seed)).unwrap();
            let typed = design
                .entities
                .iter()
                .filter(|e| e.name.contains("underground-belt"))
                .count();
            let pairs = underground_pairs(&design);
            assert_eq!(
                typed,
                pairs.len() * 2,
                "{style:?}/{seed}: unpaired underground"
            );
            for (input, output) in pairs {
                let (a, b) = (&design.entities[input], &design.entities[output]);
                assert_eq!(a.name, b.name);
                assert_eq!(a.direction, b.direction);
                let dist = (a.pos.x2 - b.pos.x2).abs().max((a.pos.y2 - b.pos.y2).abs()) / 2;
                assert!((2..=5).contains(&dist), "{style:?}/{seed}: distance {dist}");
                total += 1;
            }
        }
    }
    assert!(total > 50, "too few undergrounds generated: {total}");
}

mod random {
    use super::*;

    fn rgen(data: &RawData, seed: u64, entities: usize) -> Design {
        fsim_gen::generate(data, Params::square(32, seed).with_entities(entities)).unwrap()
    }

    #[test]
    fn places_exactly_the_requested_number_without_overlaps() {
        let Some(data) = load() else { return };
        for seed in 0..60 {
            for count in [1, 30, 80] {
                let design = rgen(&data, seed, count);
                assert_eq!(design.entities.len(), count);
                let mut taken = HashSet::new();
                for e in &design.entities {
                    let (x, y, w, h) = footprint(&data, e);
                    assert!(
                        x >= 0 && y >= 0 && x + w <= 32 && y + h <= 32,
                        "{seed}: {} outside",
                        e.name
                    );
                    for ty in y..y + h {
                        for tx in x..x + w {
                            assert!(taken.insert((tx, ty)), "{seed}: {} overlaps", e.name);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn is_deterministic_and_actually_varies() {
        let Some(data) = load() else { return };
        assert_eq!(rgen(&data, 7, 30), rgen(&data, 7, 30));
        assert_ne!(rgen(&data, 7, 30), rgen(&data, 8, 30));
        let names: HashSet<_> = (0..20)
            .flat_map(|seed| rgen(&data, seed, 30).entities)
            .map(|e| e.name.to_string())
            .collect();
        assert!(
            names.len() > 60,
            "only {} distinct entity names",
            names.len()
        );
    }

    #[test]
    fn machines_get_valid_recipes_and_underground_gets_a_side() {
        let Some(data) = load() else { return };
        let (mut machines, mut undergrounds) = (0, 0);
        for seed in 0..60 {
            for e in rgen(&data, seed, 40).entities {
                if e.name.starts_with("assembling-machine") {
                    machines += 1;
                    let recipe = e.recipe.as_ref().expect("machine without recipe");
                    assert!(data.recipe(&recipe.name).unwrap().is_some());
                }
                if e.name.contains("underground-belt") {
                    undergrounds += 1;
                    assert!(e.io.is_some());
                }
            }
        }
        assert!(machines > 0 && undergrounds > 0);
    }

    #[test]
    fn too_many_entities_is_an_error_not_a_hang() {
        let Some(data) = load() else { return };
        let err = fsim_gen::generate(&data, Params::square(4, 1).with_entities(500));
        assert!(matches!(err, Err(fsim_gen::Error::TooCrowded { .. })));
    }

    #[test]
    fn exports_and_decodes_back() {
        let Some(data) = load() else { return };
        let design = rgen(&data, 3, 30);
        let text = to_blueprint_string(&design).unwrap();
        let root = fsim_blueprint::decode(&text).unwrap();
        assert_eq!(Design::from_root(&root).unwrap(), vec![design]);
    }
}
