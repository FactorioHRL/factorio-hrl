#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("design has no entities")]
    EmptyDesign,
    #[error("entity '{0}' not found in game data")]
    UnknownEntity(String),
    #[error("sprites mode needs a game directory")]
    GameDirRequired,
    #[error("image {width}x{height} px is too large (limit {limit} px per side)")]
    ImageTooLarge { width: u64, height: u64, limit: u32 },
    #[error("invalid scale {0}: must be between 1 and 512 px per tile")]
    InvalidScale(u32),
    #[error("PNG encoding failed: {0}")]
    Png(String),
    #[error("failed to write {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error(transparent)]
    Data(#[from] fsim_data::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
