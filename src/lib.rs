//! Parses [sm-json-data], the community model of Super Metroid's layout and logic,
//! and builds the validated model in [`sm_logic`]. It accepts both upstream and Map
//! Rando's fork of the data.
//!
//! - [`load_world`] loads a checkout into a [`sm_logic::World`].
//! - [`raw`] mirrors the JSON files, for parsing and writing them.
//!
//! ```no_run
//! use sm_json_data::load_world;
//! use sm_logic::Requirement;
//!
//! let world = load_world("path/to/sm-json-data")?;
//! let room = &world[world.room_id("Morph Ball Room").unwrap()];
//! for strat in room.all_strats() {
//!     for requirement in &strat.requires {
//!         requirement.walk(&mut |r| {
//!             if let Requirement::Tech(tech) = r {
//!                 println!("{:?} needs {}", strat.name, world[*tech].name);
//!             }
//!         });
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [sm-json-data]: https://github.com/vg-json-data/sm-json-data

mod export;
mod import;
pub mod raw;

pub use export::export_world;
pub use import::{import_world, load_world, ImportErrors, ModelError, WorldError};
