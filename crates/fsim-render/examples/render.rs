use std::process::ExitCode;

use fsim_blueprint::model::Design;
use fsim_data::{GameDir, RawData};
use fsim_render::{render, Mode, Options};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [blueprint, dump, output, rest @ ..] = args.as_slice() else {
        eprintln!("usage: render <blueprint> <data-raw-dump.json> <out.png> [sprites <game-dir>] [grid] [indices] [arrows]");
        return ExitCode::FAILURE;
    };
    match run(blueprint, dump, output, rest) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn expand(path: &str) -> std::path::PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => std::path::Path::new(&home).join(rest),
        _ => path.into(),
    }
}

fn run(
    blueprint: &str,
    dump: &str,
    output: &str,
    flags: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let root = fsim_blueprint::load(expand(blueprint))?;
    let design = Design::from_root(&root)?
        .into_iter()
        .next()
        .ok_or("no blueprint found")?;
    let data = RawData::load(expand(dump))?;

    let mut options = Options::default();
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        match flag.as_str() {
            "sprites" => {
                options.mode = Mode::Sprites;
                let dir = expand(flags.next().ok_or("sprites needs a game dir")?);
                if !dir.join("data/base").is_dir() {
                    return Err(format!(
                        "'{}' is not a Factorio directory (no data/base inside); in WSL use /mnt/c/... paths",
                        dir.display()
                    )
                    .into());
                }
                options.game_dir = Some(GameDir::new(dir));
            }
            "grid" => options.grid = true,
            "indices" => options.indices = true,
            "arrows" => options.arrows = true,
            other => return Err(format!("unknown flag '{other}'").into()),
        }
    }
    let image = render(&design, &data, &options)?;
    image.save_png(expand(output))?;
    println!("{output}: {}x{} px", image.width(), image.height());
    Ok(())
}
