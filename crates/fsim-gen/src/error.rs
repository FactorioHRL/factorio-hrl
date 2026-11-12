#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unknown entity '{0}' (not in the data dump)")]
    UnknownEntity(String),
    #[error("{name} at ({x}, {y}) does not fit: {reason}")]
    Placement {
        name: String,
        x: i32,
        y: i32,
        reason: &'static str,
    },
    #[error("area {width}x{height} is too small for this style (minimum {min}x{min})")]
    TooSmall { width: i32, height: i32, min: i32 },
    #[error("placed only {placed} of {wanted} entities: the area is too crowded")]
    TooCrowded { placed: usize, wanted: usize },
    #[error("no recipe from the pool fits machine '{0}'")]
    NoRecipe(String),
    #[error(transparent)]
    Data(#[from] fsim_data::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
