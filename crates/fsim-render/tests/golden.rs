//! Golden-image tests for the schematic renderer (it needs no Wube assets, so the references
//! can live in the repo). Regenerate them with `UPDATE_GOLDEN=1 cargo test -p fsim-render --test golden`.

use std::path::PathBuf;

use fsim_blueprint::model::Design;
use fsim_data::RawData;
use fsim_render::{render, Image, Options};
use tiny_skia::Pixmap;

/// Largest allowed per-channel difference (absorbs antialiasing differences between platforms).
const CHANNEL_TOLERANCE: u8 = 8;
/// Share of pixels allowed to exceed the tolerance.
const MAX_BAD_PIXEL_SHARE: f64 = 0.005;

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.png"))
}

fn render_sample(options: &Options) -> Image {
    let data = RawData::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/data-excerpt.json"
    ))
    .unwrap();
    let root = fsim_blueprint::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fsim-blueprint/tests/sample-blueprints/example_1"
    ))
    .unwrap();
    let design = Design::from_root(&root).unwrap().remove(0);
    render(&design, &data, options).unwrap()
}

fn assert_matches_golden(name: &str, image: &Image) {
    let path = golden_path(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image.save_png(&path).unwrap();
        return;
    }
    let expected = Pixmap::load_png(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden image {} ({e}); run with UPDATE_GOLDEN=1",
            path.display()
        )
    });
    assert_eq!(
        (image.width(), image.height()),
        (expected.width(), expected.height()),
        "{name}: image size changed"
    );
    let mut bad = 0usize;
    for (x, y) in (0..image.height()).flat_map(|y| (0..image.width()).map(move |x| (x, y))) {
        let actual = image.pixel(x, y).unwrap();
        let reference = expected.pixel(x, y).unwrap().demultiply();
        let reference = [
            reference.red(),
            reference.green(),
            reference.blue(),
            reference.alpha(),
        ];
        if actual
            .iter()
            .zip(reference)
            .any(|(a, b)| a.abs_diff(b) > CHANNEL_TOLERANCE)
        {
            bad += 1;
        }
    }
    let share = bad as f64 / (image.width() * image.height()) as f64;
    assert!(
        share <= MAX_BAD_PIXEL_SHARE,
        "{name}: {bad} pixels ({:.2}%) differ from the golden image",
        share * 100.0
    );
}

#[test]
fn schematic_sample_matches_golden() {
    assert_matches_golden("example_1_schematic", &render_sample(&Options::default()));
}

#[test]
fn schematic_sample_with_overlays_matches_golden() {
    let options = Options {
        grid: true,
        indices: true,
        arrows: true,
        ..Options::default()
    };
    assert_matches_golden("example_1_overlays", &render_sample(&options));
}
