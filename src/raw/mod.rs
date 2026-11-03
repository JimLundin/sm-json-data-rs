//! A faithful mirror of the JSON files, used to parse and write them.
//!
//! Most code should use [`crate::model`] instead. Optional fields stay `Option`,
//! unknown fields are rejected, and serializing a value reproduces its file's
//! content. Schema defaults are noted on fields rather than applied.

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
