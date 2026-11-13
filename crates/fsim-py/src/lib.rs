//! Python bindings (module `factorio_rl._native`). A thin layer: every structured value crosses
//! the boundary as a JSON string, so the Python side needs no mirror of the Rust types.

use std::sync::Arc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use serde::de::DeserializeOwned;
use serde::Serialize;

use fsim_data::GameDir;
use fsim_env::{Placement, Task};
use fsim_render::{render, Mode, Options};

fn to_py<E: std::fmt::Display>(error: E) -> PyErr {
    PyValueError::new_err(error.to_string())
}

fn parse<T: DeserializeOwned>(json: &str) -> PyResult<T> {
    serde_json::from_str(json).map_err(to_py)
}

fn dump<T: Serialize>(value: &T) -> PyResult<String> {
    serde_json::to_string(value).map_err(to_py)
}

/// Game data plus the rules of the machine-layout task. Load it once and reuse it.
#[pyclass]
struct Engine {
    inner: Arc<fsim_env::Engine>,
}

/// A layout under construction (see `fsim_env::Episode`): add machines one at a time, read the
/// legal-action mask and occupancy planes without re-sending the layout.
#[pyclass]
struct Episode {
    inner: fsim_env::Episode,
}

fn bools_to_bytes<'py>(py: Python<'py>, flags: &[bool]) -> Bound<'py, PyBytes> {
    let bytes: Vec<u8> = flags.iter().copied().map(u8::from).collect();
    PyBytes::new(py, &bytes)
}

#[pymethods]
impl Episode {
    /// Builds machine `kind` (index into `Engine.kinds`) with its top left tile at `(x, y)`.
    /// Raises `ValueError` when it does not fit.
    fn place(&mut self, kind: usize, x: i32, y: i32) -> PyResult<()> {
        self.inner.place(kind, x, y).map_err(to_py)
    }

    /// One byte per (kind, y, x), 1 when a machine of that kind fits there.
    fn legal<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        bools_to_bytes(py, self.inner.legal())
    }

    /// One byte per (kind, y, x), 1 when a machine of that kind covers the tile.
    fn planes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        bools_to_bytes(py, self.inner.planes())
    }

    /// JSON `Report` of the current layout.
    fn report(&self) -> PyResult<String> {
        dump(&self.inner.report().map_err(to_py)?)
    }
}

#[pymethods]
impl Engine {
    /// `dump_path` is a `data-raw-dump.json` from a vanilla game.
    #[new]
    fn new(dump_path: &str) -> PyResult<Self> {
        Ok(Self {
            inner: Arc::new(fsim_env::Engine::load(dump_path).map_err(to_py)?),
        })
    }

    /// Starts an empty layout for a task (JSON).
    fn start(&self, task: &str) -> PyResult<Episode> {
        Ok(Episode {
            inner: fsim_env::Episode::new(Arc::clone(&self.inner), parse::<Task>(task)?)
                .map_err(to_py)?,
        })
    }

    /// JSON list of `{machine, recipe, width, height}`: the things an agent can build.
    fn kinds(&self, task: &str) -> PyResult<String> {
        dump(&self.inner.kinds(&parse::<Task>(task)?).map_err(to_py)?)
    }

    /// JSON `Report` for a layout (a JSON list of `{machine, recipe, x, y}`).
    fn evaluate(&self, task: &str, layout: &str) -> PyResult<String> {
        let report = self
            .inner
            .evaluate(&parse::<Task>(task)?, &parse::<Vec<Placement>>(layout)?)
            .map_err(to_py)?;
        dump(&report)
    }

    /// One byte per (kind, y, x), 1 when a machine of that kind fits there.
    fn legal_actions<'py>(
        &self,
        py: Python<'py>,
        task: &str,
        layout: &str,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let legal = self
            .inner
            .legal_actions(&parse::<Task>(task)?, &parse::<Vec<Placement>>(layout)?)
            .map_err(to_py)?;
        Ok(bools_to_bytes(py, &legal))
    }

    /// JSON `Report` for the machines of a blueprint string.
    fn analyze_blueprint(&self, task: &str, blueprint: &str) -> PyResult<String> {
        let report = self
            .inner
            .analyze_blueprint(&parse::<Task>(task)?, blueprint)
            .map_err(to_py)?;
        dump(&report)
    }

    /// PNG of a layout drawn by the game-like renderer. With `game_dir` (a directory holding
    /// `data/base/...`, e.g. `data/game` made by `scripts/setup-data.sh`) it uses the real sprites,
    /// otherwise schematic boxes. Only the machines' bounding box is drawn.
    #[pyo3(signature = (layout, game_dir=None, scale=32))]
    fn render_png<'py>(
        &self,
        py: Python<'py>,
        layout: &str,
        game_dir: Option<&str>,
        scale: u32,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let design = self
            .inner
            .design(&parse::<Vec<Placement>>(layout)?)
            .map_err(to_py)?;
        let mut options = Options {
            scale,
            grid: true,
            ..Options::default()
        };
        if let Some(dir) = game_dir.filter(|d| std::path::Path::new(d).join("data/base").is_dir()) {
            options.mode = Mode::Sprites;
            options.game_dir = Some(GameDir::new(dir));
        }
        let image = render(&design, self.inner.data(), &options).map_err(to_py)?;
        Ok(PyBytes::new(py, &image.to_png().map_err(to_py)?))
    }

    /// Blueprint string (to paste into the game) for a layout.
    fn export_blueprint(&self, layout: &str) -> PyResult<String> {
        self.inner
            .export_blueprint(&parse::<Vec<Placement>>(layout)?)
            .map_err(to_py)
    }
}

/// Full decoded JSON of a blueprint string, exactly as the game stores it.
#[pyfunction]
fn decode_blueprint(blueprint: &str) -> PyResult<String> {
    dump(&fsim_blueprint::decode_json(blueprint).map_err(to_py)?)
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Engine>()?;
    module.add_class::<Episode>()?;
    module.add_function(wrap_pyfunction!(decode_blueprint, module)?)?;
    Ok(())
}
