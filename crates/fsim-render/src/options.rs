use fsim_data::GameDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Schematic,
    Sprites,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub mode: Mode,
    pub scale: u32,
    pub grid: bool,
    pub indices: bool,
    pub arrows: bool,
    pub recipe_icons: bool,
    /// Bar at the bottom with the naive input/output balance (see `net_flows`).
    pub io_summary: bool,
    pub game_dir: Option<GameDir>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            mode: Mode::Schematic,
            scale: 32,
            grid: false,
            indices: false,
            arrows: false,
            recipe_icons: true,
            io_summary: false,
            game_dir: None,
        }
    }
}
