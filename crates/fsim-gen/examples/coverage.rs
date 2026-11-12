//! Checks which entities from the generator pool the renderer draws as a rectangle instead of a sprite.
//!
//! ```text
//! cargo run --release -p fsim-gen --example coverage -- <data-dir> [sheet.png]
//! ```

use fsim_blueprint::model::Direction;
use fsim_data::{GameDir, RawData};
use fsim_render::{Mode, Options, Renderer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "data".into());
    let data = RawData::load(format!("{dir}/data-raw-dump.json"))?;
    let options = Options {
        mode: Mode::Sprites,
        game_dir: Some(GameDir::new(format!("{dir}/game"))),
        scale: 8,
        ..Options::default()
    };
    let mut renderer = Renderer::new(&data);
    let names = fsim_gen::candidate_names(&data)?;
    let mut missing = Vec::new();
    for name in &names {
        let mut canvas = fsim_gen::Canvas::new(&data, 8, 8);
        canvas.place(name, 0, 0, Direction::NORTH)?;
        renderer.render(canvas.design(), &options)?;
        if !renderer.fallbacks().is_empty() {
            missing.push(name.as_str());
        }
    }
    if let Some(out) = std::env::args().nth(2) {
        // all entities on one canvas (shelf layout), for visual inspection
        let width = 48;
        let mut canvas = fsim_gen::Canvas::new(&data, width, 90);
        let (mut x, mut y, mut row) = (0, 0, 0);
        for name in &names {
            let (w, h) = canvas.size(name, Direction::NORTH)?;
            if x + w > width {
                (x, y, row) = (0, y + row + 1, 0);
            }
            canvas.place(name, x, y, Direction::NORTH)?;
            x += w + 1;
            row = row.max(h);
        }
        renderer
            .render(
                canvas.design(),
                &Options {
                    scale: 32,
                    ..options.clone()
                },
            )?
            .save_png(out)?;
    }
    println!(
        "{} of {} entities fall back to a box:",
        missing.len(),
        names.len()
    );
    for name in missing {
        println!("  {name}");
    }
    Ok(())
}
