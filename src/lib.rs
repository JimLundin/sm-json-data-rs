//! Typed Rust bindings for [sm-json-data], the community model of Super Metroid's
//! layout and logic.
//!
//! The types mirror the JSON faithfully: optional fields stay `Option`, unknown
//! fields are rejected, and serializing a value reproduces its file's content.
//! Schema defaults are noted on fields rather than applied.
//!
//! ```no_run
//! use sm_json_data::{GameData, Requirement};
//!
//! let data = GameData::load("path/to/sm-json-data")?;
//! for room in &data.rooms {
//!     for strat in room.all_strats() {
//!         for requirement in &strat.requires {
//!             requirement.walk(&mut |r| {
//!                 if let Requirement::HeatFrames(frames) = r {
//!                     println!("{}: {} needs {frames:?}", room.name, strat.name);
//!                 }
//!             });
//!         }
//!     }
//! }
//! # Ok::<(), sm_json_data::LoadError>(())
//! ```
//!
//! [sm-json-data]: https://github.com/vg-json-data/sm-json-data

mod common;
mod connection;
mod enemy;
mod game_data;
mod items;
mod logic;
mod requirement;
mod room;
mod strat;

pub use common::*;
pub use connection::*;
pub use enemy::*;
pub use game_data::*;
pub use items::*;
pub use logic::*;
pub use requirement::*;
pub use room::*;
pub use strat::*;
