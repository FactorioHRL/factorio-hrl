//! Generates blueprints and renders them right away.
//!
//! ```text
//! cargo run --release -p fsim-gen --example gen -- <data-raw-dump.json> <out-dir> [game-dir]
//! ```
//!
//! For three seeds, writes `random-<seed>.bp` (text to paste into the game) and
//! `random-<seed>.png` (sprites mode with a `game-dir`, schematic without it).

use std::path::PathBuf;
use std::process::ExitCode;

use fsim_data::{GameDir, RawData};
use fsim_gen::{generate, to_blueprint_string, Params};
use fsim_render::{Mode, Options, Renderer};

const SEEDS: [u64; 3] = [1, 2, 3];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dump, out, rest @ ..] = args.as_slice() else {
        eprintln!("usage: gen <data-raw-dump.json> <out-dir> [game-dir]");
        return ExitCode::FAILURE;
    };
    match run(dump, out, rest.first()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(dump: &str, out: &str, game_dir: Option<&String>) -> Result<(), Box<dyn std::error::Error>> {
    let data = RawData::load(dump)?;
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out)?;
    let mut options = Options::default();
    if let Some(dir) = game_dir {
        options.mode = Mode::Sprites;
        options.game_dir = Some(GameDir::new(dir));
    }
    let mut renderer = Renderer::new(&data);
    for seed in SEEDS {
        let design = generate(&data, Params::square(32, seed))?;
        let stem = format!("random-{seed}");
        std::fs::write(
            out.join(format!("{stem}.bp")),
            to_blueprint_string(&design)?,
        )?;
        let image = renderer.render(&design, &options)?;
        image.save_png(out.join(format!("{stem}.png")))?;
        println!(
            "{stem}: {} entities, {}x{} px",
            design.entities.len(),
            image.width(),
            image.height()
        );
    }
    Ok(())
}
