#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid task: {0}")]
    InvalidTask(String),
    #[error("cannot place kind {kind} at ({x}, {y}): out of bounds, overlapping, or no such kind")]
    IllegalPlacement { kind: usize, x: i32, y: i32 },
    #[error(transparent)]
    Data(#[from] fsim_data::Error),
    #[error(transparent)]
    Calc(#[from] fsim_calc::Error),
    #[error(transparent)]
    Blueprint(#[from] fsim_blueprint::Error),
    #[error(transparent)]
    Model(#[from] fsim_blueprint::model::ModelError),
}

pub type Result<T> = std::result::Result<T, Error>;
