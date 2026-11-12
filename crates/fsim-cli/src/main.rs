use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use fsim_blueprint::model::Design;
use fsim_data::{GameDir, RawData};
use fsim_gen::{legacy, Params};
use fsim_render::{Mode, Options};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(name = "fsim", about = "Factorio blueprint tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a random blueprint of the given size, e.g. `fsim gen 32x32 --png out.png`
    Gen(GenArgs),
    /// Render an existing blueprint file to PNG
    Render(RenderArgs),
    /// Decode a blueprint: print a summary (default) or the full JSON
    Decode(DecodeArgs),
    /// Print the naive input/output balance of a blueprint (items per second)
    Calc(CalcArgs),
    /// Show a typed view of a prototype from the data dump
    Inspect(InspectArgs),
}

#[derive(Args)]
struct DecodeArgs {
    /// Blueprint file, or `-` to read the blueprint string from stdin
    blueprint: String,
    /// Print the full decoded JSON instead of the summary
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct CalcArgs {
    /// Blueprint file, or `-` to read the blueprint string from stdin
    blueprint: String,
    #[command(flatten)]
    data: DataArgs,
}

#[derive(Args)]
struct InspectArgs {
    kind: InspectKind,
    /// Prototype name, e.g. `iron-gear-wheel`
    name: String,
    #[command(flatten)]
    data: DataArgs,
}

#[derive(Clone, Copy, ValueEnum)]
enum InspectKind {
    Recipe,
    Machine,
    Module,
    Item,
    Entity,
}

#[derive(Args)]
struct DataArgs {
    /// Directory with data-raw-dump.json and game/ (see scripts/setup-data.sh)
    #[arg(long, env = "FSIM_DATA", default_value = "data")]
    data: PathBuf,
}

#[derive(Args)]
struct RenderOpts {
    /// Draw simple coloured boxes instead of game sprites
    #[arg(long)]
    schematic: bool,
    /// Pixels per tile
    #[arg(long, default_value_t = 32)]
    scale: u32,
    /// Hide the bottom bar with the naive input/output balance
    #[arg(long)]
    no_summary: bool,
    /// Draw the tile grid
    #[arg(long)]
    grid: bool,
    /// Draw entity numbers
    #[arg(long)]
    indices: bool,
    /// Draw direction arrows
    #[arg(long)]
    arrows: bool,
}

#[derive(Args)]
struct GenArgs {
    /// Area size in tiles: `32x32` or `40x24` (a single number means a square)
    #[arg(default_value = "32x32", value_parser = parse_size)]
    size: (i32, i32),
    /// How many entities to place
    #[arg(short, long, default_value_t = fsim_gen::DEFAULT_ENTITIES)]
    entities: usize,
    /// Use the old template generator (belt/machine rows) with this style instead
    #[arg(long, value_enum)]
    legacy: Option<StyleArg>,
    /// Random seed (default: derived from the clock, and printed)
    #[arg(long)]
    seed: Option<u64>,
    /// Write the blueprint string here (default: print it to stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Also render the result to this PNG file
    #[arg(long)]
    png: Option<PathBuf>,
    #[command(flatten)]
    data: DataArgs,
    #[command(flatten)]
    render: RenderOpts,
}

#[derive(Args)]
struct RenderArgs {
    blueprint: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    data: DataArgs,
    #[command(flatten)]
    render: RenderOpts,
}

#[derive(Clone, Copy, ValueEnum)]
enum StyleArg {
    Random,
    Assemblers,
    Smelting,
    Chemical,
}

fn parse_size(text: &str) -> std::result::Result<(i32, i32), String> {
    let bad = || format!("'{text}' is not a size like 32x32");
    let (w, h) = text.split_once(['x', 'X']).unwrap_or((text, text));
    let (w, h): (i32, i32) = (w.parse().map_err(|_| bad())?, h.parse().map_err(|_| bad())?);
    if w < 1 || h < 1 || w > 512 || h > 512 {
        return Err("size must be between 1 and 512".into());
    }
    Ok((w, h))
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<()> {
    match command {
        Command::Gen(args) => gen(args),
        Command::Render(args) => {
            let root = fsim_blueprint::load(&args.blueprint)?;
            let design = Design::from_root(&root)?
                .into_iter()
                .next()
                .ok_or("no blueprint found")?;
            let data = load_data(&args.data.data)?;
            render(&design, &data, &args.data.data, &args.render, &args.output)
        }
        Command::Decode(args) => decode(&args),
        Command::Calc(args) => calc(&args),
        Command::Inspect(args) => inspect(&args),
    }
}

/// Reads a blueprint string from a file, or from stdin when `source` is `-`.
fn read_blueprint(source: &str) -> Result<String> {
    if source == "-" {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        Ok(text)
    } else {
        Ok(std::fs::read_to_string(source)?)
    }
}

fn decode(args: &DecodeArgs) -> Result<()> {
    let text = read_blueprint(&args.blueprint)?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&fsim_blueprint::decode_json(&text)?)?
        );
        return Ok(());
    }
    let root = fsim_blueprint::decode(&text)?;
    for (index, design) in Design::from_root(&root)?.iter().enumerate() {
        println!("blueprint {index}: {} entities", design.entities.len());
        print_counts(
            "entities",
            design.entities.iter().map(|e| e.name.to_string()),
        );
        print_counts(
            "recipes",
            design
                .entities
                .iter()
                .filter_map(|e| e.recipe.as_ref())
                .map(|r| r.name.to_string()),
        );
        print_counts(
            "modules",
            design
                .entities
                .iter()
                .flat_map(|e| &e.modules)
                .flat_map(|m| std::iter::repeat(m.item.to_string()).take(m.count as usize)),
        );
    }
    Ok(())
}

/// Prints `title:` followed by `name x count` lines, most frequent first.
fn print_counts(title: &str, names: impl Iterator<Item = String>) {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for name in names {
        *counts.entry(name).or_default() += 1;
    }
    if counts.is_empty() {
        return;
    }
    let mut rows: Vec<_> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    println!("  {title}:");
    for (name, count) in rows {
        println!("    {count:>4} x {name}");
    }
}

fn calc(args: &CalcArgs) -> Result<()> {
    let root = fsim_blueprint::decode(&read_blueprint(&args.blueprint)?)?;
    let data = load_data(&args.data.data)?;
    for (index, design) in Design::from_root(&root)?.iter().enumerate() {
        println!("blueprint {index}:");
        let flows = fsim_calc::net_flows(design, &data)?;
        for (title, wanted) in [("inputs", true), ("outputs", false)] {
            println!("  {title}:");
            for flow in flows.iter().filter(|f| f.is_input() == wanted) {
                println!("    {:>9.3}/s  {}", flow.per_second.abs(), flow.name);
            }
        }
    }
    Ok(())
}

fn inspect(args: &InspectArgs) -> Result<()> {
    let data = load_data(&args.data.data)?;
    let name = args.name.as_str();
    let found = match args.kind {
        InspectKind::Recipe => data.recipe(name)?.map(|v| format!("{v:#?}")),
        InspectKind::Machine => data.machine(name)?.map(|v| format!("{v:#?}")),
        InspectKind::Module => data.module(name)?.map(|v| format!("{v:#?}")),
        InspectKind::Item => data.item(name)?.map(|v| format!("{v:#?}")),
        InspectKind::Entity => data.entity_geometry(name)?.map(|v| format!("{v:#?}")),
    };
    println!(
        "{}",
        found.ok_or_else(|| format!("'{name}' not found in the dump"))?
    );
    Ok(())
}

fn load_data(dir: &Path) -> Result<RawData> {
    let dump = dir.join("data-raw-dump.json");
    if !dump.is_file() {
        return Err(format!(
            "'{}' not found. Copy the dump and game graphics with scripts/setup-data.sh (needs a local Factorio install) or point --data at another directory",
            dump.display()
        )
        .into());
    }
    Ok(RawData::load(dump)?)
}

fn render(
    design: &Design,
    data: &RawData,
    data_dir: &Path,
    opts: &RenderOpts,
    output: &Path,
) -> Result<()> {
    let game = data_dir.join("game");
    let mut options = Options {
        scale: opts.scale,
        io_summary: !opts.no_summary,
        grid: opts.grid,
        indices: opts.indices,
        arrows: opts.arrows,
        ..Options::default()
    };
    if !opts.schematic {
        if game.join("data/base").is_dir() {
            options.mode = Mode::Sprites;
            options.game_dir = Some(GameDir::new(game));
        } else {
            eprintln!(
                "warning: no game graphics in '{}', rendering schematic (see scripts/setup-data.sh)",
                data_dir.display()
            );
        }
    }
    let image = fsim_render::render(design, data, &options)?;
    image.save_png(output)?;
    println!(
        "{}: {}x{} px",
        output.display(),
        image.width(),
        image.height()
    );
    Ok(())
}

fn gen(args: GenArgs) -> Result<()> {
    let data = load_data(&args.data.data)?;
    let seed = args.seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64)
    });
    let (width, height) = args.size;
    let params = Params {
        width,
        height,
        seed,
        entities: args.entities,
    };
    let (design, label) = match args.legacy {
        None => (fsim_gen::generate(&data, params)?, "random".to_owned()),
        Some(choice) => {
            let style = match choice {
                StyleArg::Assemblers => legacy::Style::Assemblers,
                StyleArg::Smelting => legacy::Style::Smelting,
                StyleArg::Chemical => legacy::Style::Chemical,
                StyleArg::Random => legacy::Style::from_seed(seed),
            };
            (
                legacy::generate(&data, style, params)?,
                format!("legacy {}", style.name()),
            )
        }
    };
    eprintln!(
        "{label}, seed {seed}, {width}x{height}, {} entities",
        design.entities.len()
    );

    let text = fsim_gen::to_blueprint_string(&design)?;
    match &args.output {
        Some(path) => {
            std::fs::write(path, &text)?;
            eprintln!("blueprint written to {}", path.display());
        }
        None => println!("{text}"),
    }
    if let Some(png) = &args.png {
        render(&design, &data, &args.data.data, &args.render, png)?;
    }
    Ok(())
}
