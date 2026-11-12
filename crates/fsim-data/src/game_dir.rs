use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub const GAME_DIR_ENV: &str = "FSIM_GAME_DIR";

#[derive(Debug, Clone)]
pub struct GameDir {
    root: PathBuf,
}

impl GameDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn from_env() -> Result<Self> {
        std::env::var_os(GAME_DIR_ENV)
            .map(Self::new)
            .ok_or(Error::GameDirNotSet(GAME_DIR_ENV))
    }

    pub fn resolve(&self, path: &str) -> Result<PathBuf> {
        let unresolved = || Error::UnresolvedPath(path.to_owned());
        let rest = path.strip_prefix("__").ok_or_else(unresolved)?;
        let (module, relative) = rest.split_once("__/").ok_or_else(unresolved)?;
        if module.is_empty()
            || Path::new(relative)
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(unresolved());
        }
        Ok(self.root.join("data").join(module).join(relative))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_module_paths() {
        let dir = GameDir::new("/games/factorio");
        assert_eq!(
            dir.resolve("__base__/graphics/icons/pipe.png").unwrap(),
            Path::new("/games/factorio/data/base/graphics/icons/pipe.png")
        );
        assert_eq!(
            dir.resolve("__space-age__/a.png").unwrap(),
            Path::new("/games/factorio/data/space-age/a.png")
        );
    }

    #[test]
    fn rejects_bad_paths() {
        let dir = GameDir::new("/games/factorio");
        assert!(dir.resolve("graphics/a.png").is_err());
        assert!(dir.resolve("__base__").is_err());
        assert!(dir.resolve("__base__/../../etc/passwd").is_err());
    }
}
