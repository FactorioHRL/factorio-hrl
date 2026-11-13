//! Task, validation and scoring of machine layouts: the engine behind the RL environment.
//!
//! A [`Task`] fixes the grid, the incoming items and the allowed machines and recipes. A layout
//! is a list of [`Placement`]s. [`Engine::evaluate`] validates it and reports the steady-state
//! output ([`Report`]); [`Engine::legal_actions`] tells which machines may still be added.
//! An [`Episode`] builds a layout one machine at a time and keeps the legal-action mask up to
//! date incrementally, which is what an RL environment needs.
//! Logistics is not modelled yet (see `fsim-calc`).

mod access;
mod engine;
mod episode;
mod error;
mod links;
mod report;
mod shape;
mod task;

pub use engine::Engine;
pub use episode::Episode;
pub use error::{Error, Result};
pub use report::{FlowReport, GroupReport, PlacedView, Report, Violation};
pub use task::{Kind, Placement, Task};
