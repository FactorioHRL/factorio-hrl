use std::path::Path;

use tiny_skia::Pixmap;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Image {
    pixmap: Pixmap,
}

impl Image {
    pub(crate) fn new(pixmap: Pixmap) -> Self {
        Self { pixmap }
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let p = self.pixmap.pixel(x, y)?.demultiply();
        Some([p.red(), p.green(), p.blue(), p.alpha()])
    }

    pub fn to_png(&self) -> Result<Vec<u8>> {
        self.pixmap
            .encode_png()
            .map_err(|e| Error::Png(e.to_string()))
    }

    pub fn save_png(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        std::fs::write(path, self.to_png()?).map_err(|source| Error::Io {
            path: path.display().to_string(),
            source,
        })
    }
}
