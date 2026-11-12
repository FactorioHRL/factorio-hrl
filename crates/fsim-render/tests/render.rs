use std::path::{Path, PathBuf};

use fsim_blueprint::model::Design;
use fsim_blueprint::raw;
use fsim_data::{GameDir, RawData};
use fsim_render::{render, Error, Mode, Options};
use serde_json::{json, Value};

const SAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fsim-blueprint/tests/sample-blueprints/example_1"
);

fn excerpt() -> RawData {
    RawData::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/data-excerpt.json"
    ))
    .unwrap()
}

fn design(entities: Value) -> Design {
    let blueprint: raw::Blueprint =
        serde_json::from_value(json!({ "entities": entities })).unwrap();
    Design::from_raw(&blueprint).unwrap()
}

fn entity(number: u32, name: &str, x: f64, y: f64, direction: Option<u8>) -> Value {
    let mut e = json!({"entity_number": number, "name": name, "position": {"x": x, "y": y}});
    if let Some(d) = direction {
        e["direction"] = json!(d);
    }
    e
}

fn rgb(image: &fsim_render::Image, x: u32, y: u32) -> [u8; 3] {
    let [r, g, b, _] = image.pixel(x, y).unwrap();
    [r, g, b]
}

const BACKGROUND: [u8; 3] = [38, 38, 38];

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("fsim-render-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_solid_png(path: &Path, width: u32, height: u32, rgba: [u8; 4]) {
    let mut pixmap = tiny_skia::Pixmap::new(width, height).unwrap();
    pixmap.fill(tiny_skia::Color::from_rgba8(
        rgba[0], rgba[1], rgba[2], rgba[3],
    ));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    pixmap.save_png(path).unwrap();
}

#[test]
fn image_size_is_bounding_box_plus_one_tile_margin() {
    let d = design(json!([
        entity(1, "chemical-plant", 1.5, 1.5, None),
        entity(2, "transport-belt", 4.5, 0.5, None)
    ]));
    let image = render(&d, &excerpt(), &Options::default()).unwrap();
    assert_eq!((image.width(), image.height()), (7 * 32, 5 * 32));

    let options = Options {
        scale: 16,
        ..Options::default()
    };
    let small = render(&d, &excerpt(), &options).unwrap();
    assert_eq!((small.width(), small.height()), (7 * 16, 5 * 16));
}

#[test]
fn rotation_swaps_footprint() {
    let data = excerpt();
    let north = render(
        &design(json!([entity(1, "splitter", 1.0, 0.5, None)])),
        &data,
        &Options::default(),
    )
    .unwrap();
    let east = render(
        &design(json!([entity(1, "splitter", 0.5, 1.0, Some(4))])),
        &data,
        &Options::default(),
    )
    .unwrap();
    assert_eq!((north.width(), north.height()), (4 * 32, 3 * 32));
    assert_eq!((east.width(), east.height()), (3 * 32, 4 * 32));
}

#[test]
fn entities_are_drawn_at_their_blueprint_positions() {
    let root = fsim_blueprint::load(SAMPLE).unwrap();
    let blueprint = root.blueprints().next().unwrap();
    let design = Design::from_raw(blueprint).unwrap();
    let image = render(&design, &excerpt(), &Options::default()).unwrap();

    let data = excerpt();
    let edge = |e: &fsim_blueprint::model::Entity| {
        let geometry = data.entity_geometry(&e.name).unwrap().unwrap().geometry;
        let turned = geometry.oriented((e.direction.index() as u8 + 2) / 4 % 4);
        (
            e.pos.x() - f64::from(turned.width) / 2.0,
            e.pos.y() - f64::from(turned.height) / 2.0,
        )
    };
    let (min_x, min_y) = design
        .entities
        .iter()
        .map(edge)
        .fold((f64::MAX, f64::MAX), |(x, y), (ex, ey)| {
            (x.min(ex), y.min(ey))
        });
    let background_only = |x: u32, y: u32| rgb(&image, x, y) == BACKGROUND;
    assert!(background_only(2, 2));

    let origin_x = min_x - 1.0;
    let origin_y = min_y - 1.0;
    for e in &design.entities {
        let px = ((e.pos.x() - origin_x) * 32.0) as u32;
        let py = ((e.pos.y() - origin_y) * 32.0) as u32;
        assert!(px < image.width() && py < image.height());
        assert!(
            !background_only(px, py),
            "{} at ({}, {}) is not drawn",
            e.name,
            e.pos.x(),
            e.pos.y()
        );
    }
}

#[test]
fn same_kind_same_color_different_kind_different_color() {
    let d = design(json!([
        entity(1, "chemical-plant", 1.5, 1.5, None),
        entity(2, "oil-refinery", 6.5, 2.5, None),
        entity(3, "pipe", 10.5, 0.5, None),
        entity(4, "transport-belt", 11.5, 0.5, None)
    ]));
    let image = render(&d, &excerpt(), &Options::default()).unwrap();
    let at = |x: f64, y: f64| rgb(&image, ((x + 1.0) * 32.0) as u32, ((y + 1.0) * 32.0) as u32);
    assert_eq!(at(0.5, 0.5), at(5.0, 1.0));
    let (plant, pipe, belt) = (at(1.5, 1.5), at(10.5, 0.5), at(11.5, 0.5));
    assert_ne!(plant, pipe);
    assert_ne!(pipe, belt);
}

#[test]
fn overlays_change_the_image() {
    let d = design(json!([entity(1, "chemical-plant", 1.5, 1.5, None)]));
    let data = excerpt();
    let plain = render(&d, &data, &Options::default()).unwrap();
    for options in [
        Options {
            grid: true,
            ..Options::default()
        },
        Options {
            arrows: true,
            ..Options::default()
        },
        Options {
            indices: true,
            ..Options::default()
        },
    ] {
        let overlaid = render(&d, &data, &options).unwrap();
        assert_eq!(
            (overlaid.width(), overlaid.height()),
            (plain.width(), plain.height())
        );
        assert_ne!(overlaid.to_png().unwrap(), plain.to_png().unwrap());
    }
}

#[test]
fn output_is_deterministic() {
    let root = fsim_blueprint::load(SAMPLE).unwrap();
    let design = Design::from_raw(root.blueprints().next().unwrap()).unwrap();
    let data = excerpt();
    let options = Options {
        grid: true,
        arrows: true,
        indices: true,
        ..Options::default()
    };
    let a = render(&design, &data, &options).unwrap().to_png().unwrap();
    let b = render(&design, &data, &options).unwrap().to_png().unwrap();
    assert_eq!(a, b);
}

#[test]
fn errors() {
    let data = excerpt();
    let one = design(json!([entity(1, "pipe", 0.5, 0.5, None)]));

    assert!(matches!(
        render(&Design::default(), &data, &Options::default()),
        Err(Error::EmptyDesign)
    ));
    assert!(matches!(
        render(&design(json!([entity(1, "no-such-entity", 0.5, 0.5, None)])), &data, &Options::default()),
        Err(Error::UnknownEntity(name)) if name == "no-such-entity"
    ));
    let sprites = Options {
        mode: Mode::Sprites,
        ..Options::default()
    };
    assert!(matches!(
        render(&one, &data, &sprites),
        Err(Error::GameDirRequired)
    ));
    for scale in [0, 513] {
        let options = Options {
            scale,
            ..Options::default()
        };
        assert!(matches!(render(&one, &data, &options), Err(Error::InvalidScale(s)) if s == scale));
    }
    let far = design(json!([
        entity(1, "pipe", 0.5, 0.5, None),
        entity(2, "pipe", 5000.5, 0.5, None)
    ]));
    assert!(matches!(
        render(&far, &data, &Options::default()),
        Err(Error::ImageTooLarge { .. })
    ));
}

fn sprite_data(png: &str) -> RawData {
    RawData::from_slice(
        json!({
            "container": {"chest": {
                "type": "container", "name": "chest",
                "collision_box": [[-0.35, -0.35], [0.35, 0.35]],
                "picture": {"layers": [
                    {"filename": "__base__/graphics/entity/chest/shadow.png",
                     "width": 40, "height": 20, "scale": 0.5, "shift": [1.0, 0.0],
                     "draw_as_shadow": true},
                    {"filename": png, "width": 64, "height": 64, "scale": 0.5}
                ]}
            }}
        })
        .to_string()
        .as_bytes(),
    )
    .unwrap()
}

#[test]
fn sprites_mode_draws_layers_from_the_game_dir() {
    let dir = TempDir::new("sprites");
    let graphics = dir.path().join("data/base/graphics/entity/chest");
    write_solid_png(&graphics.join("chest.png"), 64, 64, [200, 30, 30, 255]);
    write_solid_png(&graphics.join("shadow.png"), 40, 20, [0, 0, 0, 255]);

    let data = sprite_data("__base__/graphics/entity/chest/chest.png");
    let d = design(json!([entity(1, "chest", 0.5, 0.5, None)]));
    let options = Options {
        mode: Mode::Sprites,
        game_dir: Some(GameDir::new(dir.path())),
        ..Options::default()
    };
    let image = render(&d, &data, &options).unwrap();

    assert_eq!((image.width(), image.height()), (96, 96));
    assert_eq!(rgb(&image, 48, 48), [200, 30, 30]);
    let sprite_edge = rgb(&image, 48 + 20, 48);
    assert_ne!(sprite_edge, [200, 30, 30]);
    let shadow = rgb(&image, 48 + 32, 48);
    assert!(
        shadow[0] < BACKGROUND[0],
        "shadow should darken the background: {shadow:?}"
    );
    assert!(shadow[0] > 0, "shadow should be translucent: {shadow:?}");
}

#[test]
fn sprites_mode_falls_back_to_schematic_when_the_file_is_missing() {
    let dir = TempDir::new("fallback");
    let data = sprite_data("__base__/graphics/entity/chest/missing.png");
    let d = design(json!([entity(1, "chest", 0.5, 0.5, None)]));
    let options = Options {
        mode: Mode::Sprites,
        game_dir: Some(GameDir::new(dir.path())),
        ..Options::default()
    };
    let image = render(&d, &data, &options).unwrap();
    assert_ne!(rgb(&image, 48, 48), BACKGROUND);
}

const SHEET_COLUMNS: u32 = 2;

fn belt_row_color(row: u32) -> [u8; 3] {
    [10 * (row as u8 + 1), 120, 200 - 5 * row as u8]
}

fn write_belt_sheet(path: &Path) {
    let mut pixmap = tiny_skia::Pixmap::new(SHEET_COLUMNS * 128, 20 * 128).unwrap();
    for row in 0..12 {
        let [r, g, b] = belt_row_color(row);
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(r, g, b, 255);
        let rect = tiny_skia::Rect::from_xywh(32.0, (row * 128 + 32) as f32, 64.0, 64.0).unwrap();
        pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    pixmap.save_png(path).unwrap();
}

fn belt_data() -> RawData {
    RawData::from_slice(
        json!({
            "transport-belt": {"belt": {
                "type": "transport-belt", "name": "belt",
                "collision_box": [[-0.4, -0.4], [0.4, 0.4]],
                "belt_animation_set": {"animation_set": {
                    "filename": "__base__/graphics/entity/belt/belt.png",
                    "size": 128, "scale": 0.5,
                    "frame_count": SHEET_COLUMNS, "direction_count": 20
                }}
            }}
        })
        .to_string()
        .as_bytes(),
    )
    .unwrap()
}

#[test]
fn belts_pick_straight_and_curve_variants_from_their_neighbours() {
    let dir = TempDir::new("belts");
    write_belt_sheet(&dir.path().join("data/base/graphics/entity/belt/belt.png"));

    let d = design(json!([
        entity(1, "belt", 0.5, 2.5, Some(4)),
        entity(2, "belt", 1.5, 2.5, Some(0)),
        entity(3, "belt", 1.5, 1.5, Some(0)),
        entity(4, "belt", 3.5, 0.5, Some(8))
    ]));
    let options = Options {
        mode: Mode::Sprites,
        game_dir: Some(GameDir::new(dir.path())),
        ..Options::default()
    };
    let image = render(&d, &belt_data(), &options).unwrap();

    let at = |x: f64, y: f64| rgb(&image, ((x + 1.0) * 32.0) as u32, ((y + 1.0) * 32.0) as u32);
    assert_eq!(at(0.5, 2.5), belt_row_color(0));
    assert_eq!(at(1.5, 2.5), belt_row_color(6));
    assert_eq!(at(1.5, 1.5), belt_row_color(2));
    assert_eq!(at(3.5, 0.5), belt_row_color(3));
}

const PIPE_PICTURES: [&str; 16] = [
    "straight_vertical_single",
    "ending_up",
    "ending_right",
    "corner_up_right",
    "ending_down",
    "straight_vertical",
    "corner_down_right",
    "t_right",
    "ending_left",
    "corner_up_left",
    "straight_horizontal",
    "t_up",
    "corner_down_left",
    "t_left",
    "t_down",
    "cross",
];

fn pipe_color(picture: &str) -> [u8; 3] {
    let k = PIPE_PICTURES.iter().position(|p| *p == picture).unwrap() as u8;
    [20 + 12 * k, 200 - 8 * k, 90]
}

fn write_pipe_pictures(root: &Path) {
    for picture in PIPE_PICTURES {
        let [r, g, b] = pipe_color(picture);
        let mut pixmap = tiny_skia::Pixmap::new(128, 128).unwrap();
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(r, g, b, 255);
        let rect = tiny_skia::Rect::from_xywh(32.0, 32.0, 64.0, 64.0).unwrap();
        pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
        let path = root.join(format!("data/base/graphics/entity/pipe/{picture}.png"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        pixmap.save_png(path).unwrap();
    }
}

fn pipe_data() -> RawData {
    let pictures: serde_json::Map<String, Value> = PIPE_PICTURES
        .iter()
        .map(|name| {
            let spec = json!({
                "filename": format!("__base__/graphics/entity/pipe/{name}.png"),
                "width": 128, "height": 128, "scale": 0.5
            });
            (name.to_string(), spec)
        })
        .collect();
    let port = |direction: u8, x: f64, y: f64| json!({"direction": direction, "position": [x, y]});
    RawData::from_slice(
        json!({
            "pipe": {"pipe": {
                "type": "pipe", "name": "pipe",
                "collision_box": [[-0.29, -0.29], [0.29, 0.29]],
                "fluid_box": {"pipe_connections": [
                    port(0, 0.0, 0.0), port(4, 0.0, 0.0), port(8, 0.0, 0.0), port(12, 0.0, 0.0)
                ]},
                "pictures": pictures
            }},
            "pipe-to-ground": {"pipe-to-ground": {
                "type": "pipe-to-ground", "name": "pipe-to-ground",
                "collision_box": [[-0.29, -0.29], [0.29, 0.29]],
                "fluid_box": {"pipe_connections": [
                    port(0, 0.0, 0.0),
                    {"direction": 8, "position": [0, 0], "connection_type": "underground"}
                ]}
            }},
            "assembling-machine": {"chemical-plant": {
                "type": "assembling-machine", "name": "chemical-plant",
                "collision_box": [[-1.2, -1.2], [1.2, 1.2]],
                "crafting_categories": ["chemistry"],
                "fluid_boxes": [
                    {"production_type": "input", "pipe_connections": [port(0, -1.0, -1.0)]},
                    {"production_type": "input", "pipe_connections": [port(0, 1.0, -1.0)]},
                    {"production_type": "output", "pipe_connections": [port(8, -1.0, 1.0)]},
                    {"production_type": "output", "pipe_connections": [port(8, 1.0, 1.0)]}
                ]
            }},
            "recipe": {"plastic-bar": {
                "type": "recipe", "name": "plastic-bar",
                "ingredients": [{"type": "fluid", "name": "petroleum-gas", "amount": 20},
                                {"type": "item", "name": "coal", "amount": 1}],
                "results": [{"type": "item", "name": "plastic-bar", "amount": 2}]
            }}
        })
        .to_string()
        .as_bytes(),
    )
    .unwrap()
}

#[test]
fn pipes_pick_their_picture_from_neighbours_pipe_to_ground_and_machine_ports() {
    let dir = TempDir::new("pipes");
    write_pipe_pictures(dir.path());

    let mut plant = entity(8, "chemical-plant", 8.5, 1.5, None);
    plant["recipe"] = json!("plastic-bar");
    let d = design(json!([
        entity(1, "pipe", 0.5, 0.5, None),
        entity(2, "pipe", 1.5, 0.5, None),
        entity(3, "pipe", 1.5, 1.5, None),
        entity(4, "pipe", 3.5, -0.5, None),
        entity(5, "pipe-to-ground", 3.5, 0.5, None),
        entity(6, "pipe", 3.5, 1.5, None),
        plant,
        entity(9, "pipe", 7.5, -0.5, None),
        entity(10, "pipe", 10.5, 1.5, None),
        entity(11, "pipe", 9.5, -0.5, None)
    ]));
    let options = Options {
        mode: Mode::Sprites,
        game_dir: Some(GameDir::new(dir.path())),
        ..Options::default()
    };
    let image = render(&d, &pipe_data(), &options).unwrap();

    let origin = (-1.0, -2.0);
    let at = |x: f64, y: f64| {
        rgb(
            &image,
            ((x - origin.0) * 32.0) as u32,
            ((y - origin.1) * 32.0) as u32,
        )
    };
    assert_eq!(at(0.5, 0.5), pipe_color("ending_right"));
    assert_eq!(at(1.5, 0.5), pipe_color("corner_down_left"));
    assert_eq!(at(1.5, 1.5), pipe_color("ending_up"));
    assert_eq!(at(3.5, -0.5), pipe_color("ending_down"));
    assert_eq!(at(3.5, 1.5), pipe_color("straight_vertical_single"));
    assert_eq!(at(7.5, -0.5), pipe_color("ending_down"));
    assert_eq!(at(10.5, 1.5), pipe_color("straight_vertical_single"));
    assert_eq!(at(9.5, -0.5), pipe_color("ending_down"));
}

const PLATFORM_COLORS: [[u8; 3]; 4] = [[200, 50, 50], [50, 200, 50], [50, 50, 200], [200, 200, 50]];
const HAND_COLOR: [u8; 3] = [250, 250, 250];
const BASE_COLOR: [u8; 3] = [30, 120, 250];

fn write_inserter_images(root: &Path) {
    let dir = root.join("data/base/graphics/entity/ins");
    let mut platform = tiny_skia::Pixmap::new(4 * 105, 79).unwrap();
    for (frame, [r, g, b]) in PLATFORM_COLORS.iter().enumerate() {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(*r, *g, *b, 255);
        let rect = tiny_skia::Rect::from_xywh(frame as f32 * 105.0, 0.0, 105.0, 79.0).unwrap();
        platform.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
    }
    std::fs::create_dir_all(&dir).unwrap();
    platform.save_png(dir.join("platform.png")).unwrap();
    write_solid_png(
        &dir.join("hand.png"),
        72,
        164,
        [HAND_COLOR[0], HAND_COLOR[1], HAND_COLOR[2], 255],
    );
    write_solid_png(
        &dir.join("base.png"),
        32,
        136,
        [BASE_COLOR[0], BASE_COLOR[1], BASE_COLOR[2], 255],
    );
}

fn inserter_data() -> RawData {
    let sprite = |name: &str, w: u32, h: u32, scale: f64| {
        json!({"filename": format!("__base__/graphics/entity/ins/{name}.png"),
               "width": w, "height": h, "scale": scale})
    };
    let inserter = |pickup: f64| {
        json!({
            "type": "inserter",
            "collision_box": [[-0.15, -0.15], [0.15, 0.15]],
            "pickup_position": [0, pickup],
            "platform_picture": {"sheet": sprite("platform", 105, 79, 0.5)},
            "hand_base_picture": sprite("base", 32, 136, 0.25),
            "hand_open_picture": sprite("hand", 72, 164, 0.25)
        })
    };
    let (mut short, mut long) = (inserter(-1.0), inserter(-2.0));
    short["name"] = json!("inserter");
    long["name"] = json!("long-handed-inserter");
    RawData::from_slice(
        json!({"inserter": {"inserter": short, "long-handed-inserter": long}})
            .to_string()
            .as_bytes(),
    )
    .unwrap()
}

fn render_inserter(name: &str, direction: u8, dir: &Path) -> fsim_render::Image {
    let d = design(json!([entity(1, name, 0.5, 0.5, Some(direction))]));
    let options = Options {
        mode: Mode::Sprites,
        game_dir: Some(GameDir::new(dir)),
        ..Options::default()
    };
    render(&d, &inserter_data(), &options).unwrap()
}

#[test]
fn inserter_arm_reaches_towards_the_pickup_side() {
    let dir = TempDir::new("inserters");
    write_inserter_images(dir.path());

    let south = render_inserter("inserter", 8, dir.path());
    assert_eq!((south.width(), south.height()), (96, 96));
    assert_eq!(rgb(&south, 48, 28), HAND_COLOR);
    assert_ne!(rgb(&south, 48, 70), HAND_COLOR);

    let west = render_inserter("inserter", 12, dir.path());
    assert_eq!(rgb(&west, 68, 48), HAND_COLOR);
    assert_ne!(rgb(&west, 28, 48), HAND_COLOR);

    let east = render_inserter("inserter", 4, dir.path());
    assert_eq!(rgb(&east, 28, 48), HAND_COLOR);
    assert_ne!(rgb(&east, 68, 48), HAND_COLOR);

    let north = render_inserter("inserter", 0, dir.path());
    assert_eq!(rgb(&north, 48, 68), HAND_COLOR);
}

#[test]
fn inserter_platform_frame_follows_the_direction() {
    let dir = TempDir::new("inserter-platform");
    write_inserter_images(dir.path());
    for (direction, frame) in [(0, 0), (4, 1), (8, 2), (12, 3)] {
        let image = render_inserter("inserter", direction, dir.path());
        assert_eq!(
            rgb(&image, 48 + 20, 48 + 14),
            PLATFORM_COLORS[frame],
            "direction {direction}"
        );
    }
}

#[test]
fn long_handed_inserter_has_a_longer_arm_with_the_base_bridging_the_gap() {
    let dir = TempDir::new("long-inserter");
    write_inserter_images(dir.path());
    let long = render_inserter("long-handed-inserter", 8, dir.path());
    assert_eq!(rgb(&long, 48, 48 - 40), HAND_COLOR);
    assert_eq!(rgb(&long, 48 + 2, 48 - 20), BASE_COLOR);

    let short = render_inserter("inserter", 8, dir.path());
    assert_ne!(rgb(&short, 48 + 2, 48 - 20), BASE_COLOR);
}

#[test]
fn recipe_icon_is_drawn_when_a_game_dir_is_given() {
    let dir = TempDir::new("icons");
    write_solid_png(
        &dir.path().join("data/base/graphics/icons/plastic-bar.png"),
        120,
        64,
        [250, 250, 20, 255],
    );
    let data = excerpt();
    let mut plant = entity(1, "chemical-plant", 1.5, 1.5, None);
    plant["recipe"] = json!("plastic-bar");
    let d = design(json!([plant]));

    let without = render(&d, &data, &Options::default()).unwrap();
    let options = Options {
        game_dir: Some(GameDir::new(dir.path())),
        ..Options::default()
    };
    let with = render(&d, &data, &options).unwrap();

    let center = (2.5 * 32.0) as u32;
    assert_ne!(rgb(&without, center, center), [250, 250, 20]);
    assert_eq!(rgb(&with, center, center), [250, 250, 20]);

    let hidden = Options {
        recipe_icons: false,
        ..options
    };
    assert_eq!(
        render(&d, &data, &hidden).unwrap().to_png().unwrap(),
        without.to_png().unwrap()
    );
}

#[test]
fn real_game_assets_render_the_sample() {
    let (Some(dump), Some(game_dir)) = (
        std::env::var_os("FSIM_DUMP"),
        std::env::var_os(fsim_data::GAME_DIR_ENV),
    ) else {
        eprintln!("FSIM_DUMP or FSIM_GAME_DIR not set, skipping");
        return;
    };
    let data = RawData::load(dump).unwrap();
    let root = fsim_blueprint::load(SAMPLE).unwrap();
    let design = Design::from_raw(root.blueprints().next().unwrap()).unwrap();
    let options = Options {
        mode: Mode::Sprites,
        game_dir: Some(GameDir::new(game_dir)),
        ..Options::default()
    };
    let started = std::time::Instant::now();
    let image = render(&design, &data, &options).unwrap();
    eprintln!("rendered in {:?}", started.elapsed());
    assert!(image.width() > 0 && image.height() > 0);
    if let Some(out) = std::env::var_os("FSIM_RENDER_OUT") {
        image.save_png(out).unwrap();
    }
}

#[test]
fn io_summary_adds_a_bar_below_the_map_only_when_asked() {
    let d = design(json!([
        {"entity_number": 1, "name": "chemical-plant", "position": {"x": 1.5, "y": 1.5}, "recipe": "plastic-bar"},
        entity(2, "pipe", 4.5, 0.5, None),
    ]));
    let data = excerpt();
    let plain = render(&d, &data, &Options::default()).unwrap();
    let with = render(
        &d,
        &data,
        &Options {
            io_summary: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(with.width(), plain.width());
    assert!(with.height() > plain.height());
    // the bar starts below the map: the map background differs from the bar background
    assert_ne!(plain.pixel(2, 2), with.pixel(2, plain.height() + 10));
    // the map above the bar is the same
    assert_eq!(plain.pixel(40, 40), with.pixel(40, 40));
}

#[test]
fn io_summary_without_recipes_changes_nothing() {
    let d = design(json!([entity(1, "pipe", 0.5, 0.5, None)]));
    let data = excerpt();
    let plain = render(&d, &data, &Options::default()).unwrap();
    let with = render(
        &d,
        &data,
        &Options {
            io_summary: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(plain.to_png().unwrap(), with.to_png().unwrap());
}
