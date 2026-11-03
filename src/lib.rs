//! Rust bindings for [sm-json-data], the community model of Super Metroid's layout
//! and logic. They accept both upstream and Map Rando's fork of the data.
//!
//! - [`model`] is the domain model most code should use, built with [`World::load`].
//! - [`raw`] mirrors the JSON files, for parsing and writing them.
//!
//! ```no_run
//! use sm_json_data::model::{Requirement, World};
//!
//! let world = World::load("path/to/sm-json-data")?;
//! let room = &world[world.room_id("Morph Ball Room").unwrap()];
//! for strat in room.all_strats() {
//!     for requirement in &strat.requires {
//!         requirement.walk(&mut |r| {
//!             if let Requirement::Tech(tech) = r {
//!                 println!("{} needs {}", strat.name, world[*tech].name);
//!             }
//!         });
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [sm-json-data]: https://github.com/vg-json-data/sm-json-data
//! [`World::load`]: model::World::load

pub mod model;
pub mod raw;
