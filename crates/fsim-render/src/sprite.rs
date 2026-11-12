use fsim_blueprint::model::{Direction, IoType};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpriteLayer {
    pub filename: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub shift: (f64, f64),
    pub shadow: bool,
}

const DIRECTION_KEYS: [&str; 4] = ["north", "east", "south", "west"];
const SOURCES: [&str; 13] = [
    "picture",
    "pictures",
    "animation",
    "sprite",
    "idle_animation",
    "off_animation",
    "base_picture",
    "structure",
    "animations",
    "chargable_graphics",
    "picture_off",
    "connection_sprites",
    "robot_door",
];

/// Keys under which a sprite may be nested one level deep (e.g. `pictures.picture`,
/// `pictures.north.structure`, `pictures.north.animation`, `pictures.single`).
const WRAPPERS: [&str; 4] = ["picture", "structure", "animation", "single"];

pub(crate) fn entity_layers(prototype: &Value, direction: Direction) -> Vec<SpriteLayer> {
    // The tank has two frames (vertical and horizontal), chosen by quarter-turn parity.
    if prototype.get("type").and_then(Value::as_str) == Some("storage-tank") {
        let turned = crate::layout::quarter_turns(direction) % 2 == 1;
        let direction = if turned {
            Direction::SOUTH
        } else {
            Direction::NORTH
        };
        return named_layers(prototype, "pictures", direction);
    }
    // Turret: base from `graphics_set` and the folded gun.
    if let Some(folded) = prototype.get("folded_animation") {
        let base = prototype
            .get("graphics_set")
            .and_then(|set| set.get("base_visualisation"))
            .and_then(|visual| visual.get("animation"))
            .map(|animation| resolve(animation, direction))
            .unwrap_or_default();
        return [base, resolve(folded, direction)].concat();
    }
    // Roboport: base, patch and antenna.
    if prototype.get("base_patch").is_some() {
        return ["base", "base_patch", "base_animation"]
            .iter()
            .flat_map(|key| named_layers(prototype, key, direction))
            .collect();
    }
    if let Some(set) = prototype.get("graphics_set") {
        if let Some(Value::Array(list)) = set.get("animation_list") {
            let layers: Vec<_> = list
                .iter()
                .filter(|item| item.get("always_draw").and_then(Value::as_bool) != Some(false))
                .filter_map(|item| item.get("animation"))
                .flat_map(|animation| resolve(animation, direction))
                .collect();
            if !layers.is_empty() {
                return layers;
            }
        }
        for key in ["animation", "idle_animation"] {
            if let Some(animation) = set.get(key) {
                let layers = resolve(animation, direction);
                if !layers.is_empty() {
                    return layers;
                }
            }
        }
    }
    SOURCES
        .iter()
        .filter_map(|key| prototype.get(key))
        .map(|spec| resolve(spec, direction))
        .find(|layers| !layers.is_empty())
        .unwrap_or_default()
}

fn resolve(spec: &Value, direction: Direction) -> Vec<SpriteLayer> {
    match spec {
        Value::Array(variants) => variants
            .first()
            .map(|first| resolve(first, direction))
            .unwrap_or_default(),
        Value::Object(map) => {
            if let Some(Value::Array(layers)) = map.get("layers") {
                return layers.iter().flat_map(|l| resolve(l, direction)).collect();
            }
            if let Some(Value::Array(sheets)) = map.get("sheets") {
                return sheets.iter().filter_map(|s| sheet(s, direction)).collect();
            }
            if let Some(single) = map.get("sheet") {
                return sheet(single, direction).into_iter().collect();
            }
            if DIRECTION_KEYS.iter().any(|k| map.contains_key(*k)) {
                let quarter = crate::layout::quarter_turns(direction) as usize;
                let key = DIRECTION_KEYS[quarter];
                let opposite = DIRECTION_KEYS[(quarter + 2) % 4];
                return map
                    .get(key)
                    .or_else(|| map.get(opposite))
                    .or_else(|| map.get("north"))
                    .map(|chosen| resolve(chosen, direction))
                    .unwrap_or_default();
            }
            if !map.contains_key("filename") {
                for key in WRAPPERS {
                    if let Some(inner) = map.get(key) {
                        let layers = resolve(inner, direction);
                        if !layers.is_empty() {
                            return layers;
                        }
                    }
                }
            }
            sprite(spec, direction).into_iter().collect()
        }
        _ => Vec::new(),
    }
}

pub(crate) fn pair(value: &Value) -> Option<(f64, f64)> {
    match value {
        Value::Array(xy) => Some((xy.first()?.as_f64()?, xy.get(1)?.as_f64()?)),
        Value::Object(xy) => Some((xy.get("x")?.as_f64()?, xy.get("y")?.as_f64()?)),
        _ => None,
    }
}

fn number(spec: &Value, key: &str) -> Option<u32> {
    spec.get(key)?.as_u64().and_then(|n| u32::try_from(n).ok())
}

fn flag(spec: &Value, key: &str) -> bool {
    spec.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Layers with the `mask` flag are player-colour masks (applied with a tint); on their own they would look
/// like a white blob.
fn is_mask(spec: &Value) -> bool {
    spec.get("flags")
        .and_then(Value::as_array)
        .is_some_and(|flags| flags.iter().any(|f| f == "mask"))
}

fn variant_for(direction: Direction, count: u32) -> u32 {
    if count > 1 {
        direction.index() as u32 * count / u32::from(Direction::COUNT)
    } else {
        0
    }
}

fn sprite(spec: &Value, direction: Direction) -> Option<SpriteLayer> {
    let count = number(spec, "direction_count").unwrap_or(1).max(1);
    sprite_at(spec, variant_for(direction, count), count)
}

fn sheet(spec: &Value, direction: Direction) -> Option<SpriteLayer> {
    let count = number(spec, "direction_count")
        .or_else(|| number(spec, "frames"))
        .unwrap_or(4)
        .max(1);
    sprite_at(spec, variant_for(direction, count), count)
}

fn sprite_variant(spec: &Value, variant: u32) -> Option<SpriteLayer> {
    let count = number(spec, "direction_count").unwrap_or(1).max(1);
    sprite_at(spec, variant, count)
}

fn sprite_at(spec: &Value, variant: u32, direction_count: u32) -> Option<SpriteLayer> {
    let filename = spec.get("filename")?.as_str()?.to_owned();
    if flag(spec, "draw_as_glow") || flag(spec, "draw_as_light") || is_mask(spec) {
        return None;
    }
    let (width, height) = match (number(spec, "width"), number(spec, "height")) {
        (Some(w), Some(h)) => (w, h),
        _ => {
            let (w, h) = match spec.get("size")? {
                Value::Number(n) => (n.as_f64()?, n.as_f64()?),
                other => pair(other)?,
            };
            (w as u32, h as u32)
        }
    };
    let frame_count = number(spec, "frame_count").unwrap_or(1).max(1);
    let default_line_length = if frame_count > 1 {
        frame_count
    } else {
        direction_count
    };
    let line_length = number(spec, "line_length")
        .filter(|&n| n > 0)
        .unwrap_or(default_line_length);
    let frame = variant * frame_count;
    Some(SpriteLayer {
        filename,
        x: number(spec, "x").unwrap_or(0) + (frame % line_length) * width,
        y: number(spec, "y").unwrap_or(0) + (frame / line_length) * height,
        width,
        height,
        scale: spec.get("scale").and_then(Value::as_f64).unwrap_or(1.0),
        shift: spec.get("shift").and_then(pair).unwrap_or((0.0, 0.0)),
        shadow: flag(spec, "draw_as_shadow"),
    })
}

pub(crate) fn named_layers(prototype: &Value, key: &str, direction: Direction) -> Vec<SpriteLayer> {
    prototype
        .get(key)
        .map(|spec| resolve(spec, direction))
        .unwrap_or_default()
}

pub(crate) fn pipe_layers(prototype: &Value, picture: &str) -> Vec<SpriteLayer> {
    prototype
        .get("pictures")
        .and_then(|pictures| pictures.get(picture))
        .map(|spec| resolve(spec, Direction::NORTH))
        .unwrap_or_default()
}

pub(crate) fn belt_layers(prototype: &Value, variants: &[u8]) -> Vec<SpriteLayer> {
    let Some(spec) = prototype
        .get("belt_animation_set")
        .and_then(|set| set.get("animation_set"))
    else {
        return Vec::new();
    };
    variants
        .iter()
        .filter_map(|&index| sprite_variant(spec, u32::from(index).checked_sub(1)?))
        .collect()
}

/// Underground belt: lane underneath, then the entrance/exit arc (`direction_in`/`direction_out`)
/// with patches (`back_patch` behind, `front_patch` in front of it).
pub(crate) fn underground_layers(
    prototype: &Value,
    direction: Direction,
    io: Option<IoType>,
    straight_variant: u8,
) -> Vec<SpriteLayer> {
    let part = |key: &str| {
        prototype
            .get("structure")
            .and_then(|structure| structure.get(key))
            .map(|spec| resolve(spec, direction))
            .unwrap_or_default()
    };
    let body = match io {
        Some(IoType::Output) => "direction_out",
        _ => "direction_in",
    };
    let mut layers = belt_layers(prototype, &[straight_variant]);
    layers.extend(part("back_patch"));
    layers.extend(part(body));
    layers.extend(part("front_patch"));
    layers
}

/// Splitter: two lanes underneath (one per tile, from `belt_animation_set`), the body on them
/// (`structure`), and `structure_patch` on top (for E/W it covers the upper part of the body).
pub(crate) fn splitter_layers(
    prototype: &Value,
    direction: Direction,
    straight_variant: u8,
) -> Vec<SpriteLayer> {
    let along_x = crate::layout::quarter_turns(direction) % 2 == 0;
    let mut layers = Vec::new();
    for side in [-0.5, 0.5] {
        for mut belt in belt_layers(prototype, &[straight_variant]) {
            belt.shift = if along_x {
                (belt.shift.0 + side, belt.shift.1)
            } else {
                (belt.shift.0, belt.shift.1 + side)
            };
            layers.push(belt);
        }
    }
    layers.extend(named_layers(prototype, "structure", direction));
    layers.extend(named_layers(prototype, "structure_patch", direction));
    layers
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn layers(spec: Value, direction: Direction) -> Vec<SpriteLayer> {
        entity_layers(&spec, direction)
    }

    #[test]
    fn plain_layers_with_shadow_and_shift() {
        let out = layers(
            json!({"picture": {"layers": [
                {"filename": "a.png", "width": 62, "height": 72, "scale": 0.5, "shift": [0.25, -0.5]},
                {"filename": "s.png", "width": 104, "height": 40, "draw_as_shadow": true}
            ]}}),
            Direction::NORTH,
        );
        assert_eq!(out.len(), 2);
        assert_eq!((out[0].width, out[0].height, out[0].scale), (62, 72, 0.5));
        assert_eq!(out[0].shift, (0.25, -0.5));
        assert!(!out[0].shadow);
        assert!(out[1].shadow);
        assert_eq!(out[1].scale, 1.0);
    }

    #[test]
    fn graphics_set_animation_takes_first_frame_of_sheet() {
        let out = layers(
            json!({"graphics_set": {"animation": {"layers": [
                {"filename": "anim.png", "width": 148, "height": 158, "frame_count": 64, "line_length": 8}
            ]}}}),
            Direction::NORTH,
        );
        assert_eq!((out[0].x, out[0].y), (0, 0));
    }

    #[test]
    fn four_way_graphics_pick_the_direction() {
        let spec = json!({"graphics_set": {"animation": {
            "north": {"layers": [{"filename": "n.png", "width": 1, "height": 1}]},
            "east": {"layers": [{"filename": "e.png", "width": 1, "height": 1}]},
            "south": {"layers": [{"filename": "s.png", "width": 1, "height": 1}]},
            "west": {"layers": [{"filename": "w.png", "width": 1, "height": 1}]}
        }}});
        let name = |d| layers(spec.clone(), d)[0].filename.clone();
        assert_eq!(name(Direction::NORTH), "n.png");
        assert_eq!(name(Direction::EAST), "e.png");
        assert_eq!(name(Direction::SOUTH), "s.png");
        assert_eq!(name(Direction::WEST), "w.png");
    }

    #[test]
    fn rotated_sprite_selects_frame_by_direction() {
        let spec = json!({"pictures": {"layers": [
            {"filename": "p.png", "width": 72, "height": 220, "direction_count": 4}
        ]}});
        assert_eq!(layers(spec.clone(), Direction::NORTH)[0].x, 0);
        assert_eq!(layers(spec.clone(), Direction::EAST)[0].x, 72);
        assert_eq!(layers(spec, Direction::WEST)[0].x, 3 * 72);
    }

    #[test]
    fn animated_rotated_sheet_has_one_row_per_direction() {
        let spec = json!({"animation": {"filename": "a.png", "size": 128,
            "frame_count": 16, "direction_count": 20}});
        let out = layers(spec, Direction::EAST);
        assert_eq!((out[0].x, out[0].y), (0, 5 * 128));
    }

    #[test]
    fn belt_variants_are_rows_of_the_animation_set() {
        let belt = json!({"belt_animation_set": {"animation_set": {
            "filename": "belt.png", "size": 128, "scale": 0.5,
            "frame_count": 16, "direction_count": 20}}});
        let out = belt_layers(&belt, &[1, 7, 20]);
        let rows: Vec<_> = out.iter().map(|l| (l.x, l.y)).collect();
        assert_eq!(rows, [(0, 0), (0, 6 * 128), (0, 19 * 128)]);
        assert!(belt_layers(&belt, &[0]).is_empty());
        assert!(belt_layers(&json!({}), &[1]).is_empty());
    }

    #[test]
    fn pipe_picture_is_looked_up_by_name() {
        let pipe = json!({"pictures": {
            "ending_up": {"filename": "up.png", "width": 128, "height": 128, "scale": 0.5},
            "cross": {"filename": "cross.png", "width": 128, "height": 128}
        }});
        assert_eq!(pipe_layers(&pipe, "ending_up")[0].filename, "up.png");
        assert_eq!(pipe_layers(&pipe, "cross")[0].filename, "cross.png");
        assert!(pipe_layers(&pipe, "t_up").is_empty());
        assert!(pipe_layers(&json!({}), "cross").is_empty());
    }

    #[test]
    fn four_way_sheet_has_one_frame_per_direction_by_default() {
        let spec = json!({"platform_picture": {"sheet": {"filename": "p.png", "width": 105, "height": 79}}});
        let x = |d| named_layers(&spec, "platform_picture", d)[0].x;
        assert_eq!(x(Direction::NORTH), 0);
        assert_eq!(x(Direction::EAST), 105);
        assert_eq!(x(Direction::SOUTH), 210);
        assert_eq!(x(Direction::WEST), 315);
    }

    #[test]
    fn sheets_and_explicit_frames() {
        let spec = json!({"picture": {"sheets": [
            {"filename": "a.png", "width": 10, "height": 10},
            {"filename": "b.png", "width": 10, "height": 10, "frames": 1, "draw_as_shadow": true}
        ]}});
        let out = layers(spec, Direction::EAST);
        assert_eq!((out[0].filename.as_str(), out[0].x), ("a.png", 10));
        assert_eq!((out[1].filename.as_str(), out[1].x), ("b.png", 0));
    }

    #[test]
    fn rotated_sprite_wraps_into_rows() {
        let spec = json!({"picture": {"filename": "p.png", "width": 10, "height": 20,
            "direction_count": 8, "line_length": 4}});
        let out = layers(spec, Direction::SOUTH);
        assert_eq!((out[0].x, out[0].y), (0, 20));
    }

    #[test]
    fn glow_layers_and_missing_files_are_skipped() {
        let out = layers(
            json!({"animation": {"layers": [
                {"filename": "glow.png", "width": 1, "height": 1, "draw_as_glow": true},
                {"width": 1, "height": 1},
                {"filename": "ok.png", "size": 64}
            ]}}),
            Direction::NORTH,
        );
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].width, out[0].height), (64, 64));
    }

    #[test]
    fn animation_list_keeps_always_drawn_entries() {
        let out = layers(
            json!({"graphics_set": {"animation_list": [
                {"always_draw": true, "animation": {"filename": "bottom.png", "width": 1, "height": 1}},
                {"always_draw": false, "animation": {"filename": "modules.png", "width": 1, "height": 1}},
                {"animation": {"filename": "top.png", "width": 1, "height": 1}}
            ]}}),
            Direction::NORTH,
        );
        let names: Vec<_> = out.iter().map(|l| l.filename.as_str()).collect();
        assert_eq!(names, ["bottom.png", "top.png"]);
    }

    #[test]
    fn entities_without_graphics_give_nothing() {
        assert!(layers(json!({"icon": "x.png"}), Direction::NORTH).is_empty());
        assert!(layers(json!({"picture": {"stripes": []}}), Direction::NORTH).is_empty());
    }

    #[test]
    fn splitter_has_a_belt_per_tile_then_body_then_patch() {
        let splitter = json!({
            "belt_animation_set": {"animation_set": {
                "filename": "belt.png", "width": 64, "height": 64, "frame_count": 16,
                "line_length": 16, "scale": 0.5
            }},
            "structure": {"north": {"filename": "n.png", "width": 10, "height": 10},
                          "east": {"filename": "e.png", "width": 10, "height": 10}},
            "structure_patch": {"north": {"filename": "empty.png", "width": 1, "height": 1},
                                "east": {"filename": "patch.png", "width": 10, "height": 10}},
        });
        let north = splitter_layers(&splitter, Direction::NORTH, 3);
        let names: Vec<_> = north.iter().map(|l| l.filename.as_str()).collect();
        assert_eq!(names, ["belt.png", "belt.png", "n.png", "empty.png"]);
        assert_eq!((north[0].shift, north[1].shift), ((-0.5, 0.0), (0.5, 0.0)));
        assert_eq!(north[0].y, 2 * 64, "variant 3 is the third row");

        let east = splitter_layers(&splitter, Direction::EAST, 1);
        assert_eq!((east[0].shift, east[1].shift), ((0.0, -0.5), (0.0, 0.5)));
        assert_eq!(east.last().unwrap().filename, "patch.png");
    }

    #[test]
    fn underground_picks_input_or_output_sheet_and_frame() {
        let sheet =
            |y: u32| json!({"sheet": {"filename": "u.png", "width": 192, "height": 192, "y": y}});
        let underground = json!({
            "belt_animation_set": {"animation_set": {
                "filename": "belt.png", "size": 128, "frame_count": 16, "direction_count": 20
            }},
            "structure": {
                "direction_in": sheet(192), "direction_out": sheet(0),
                "back_patch": sheet(1000), "front_patch": sheet(2000)
            },
        });
        let input = underground_layers(&underground, Direction::EAST, Some(IoType::Input), 1);
        let ys: Vec<_> = input.iter().map(|l| l.y).collect();
        assert_eq!(ys, [0, 1000, 192, 2000]);
        assert!(
            input[1..].iter().all(|l| l.x == 192),
            "east is the second frame"
        );
        let output = underground_layers(&underground, Direction::EAST, Some(IoType::Output), 1);
        assert_eq!(output[2].y, 0);
    }
}
