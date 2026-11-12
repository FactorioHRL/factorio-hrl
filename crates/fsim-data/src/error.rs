#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid prototype {kind}/{name}: {source}")]
    Prototype {
        kind: String,
        name: String,
        source: serde_json::Error,
    },
    #[error("invalid energy value '{0}'")]
    InvalidEnergy(String),
    #[error("prototype {kind}/{name} has no collision_box")]
    NoCollisionBox { kind: String, name: String },
    #[error("cannot resolve path '{0}'")]
    UnresolvedPath(String),
    #[error("game directory not set: pass it explicitly or set {0}")]
    GameDirNotSet(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;
