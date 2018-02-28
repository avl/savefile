#![feature(alloc)]
#![feature(allocator_api)]
#![recursion_limit = "256"]
#![feature(test)]
#![feature(specialization)]
#![feature(attr_literals)]

//! This is the documentation for `savefile`
//!
//! # Introduction
//!
//! Savefile is a rust library to conveniently, quickly and correctly
//! serialize and deserialize arbitrary rust struct and enums into
//! an efficient and compact binary version controlled format.
//!
//! The design use case is any application that needs to save large
//! amounts of data to disk, and support loading files from previous
//! versions of the program (but not from later versions!).
//!
//!
//! # Example
//!
//! ```
//! extern crate savefile;
//! use savefile::prelude::*;
//!
//! #[macro_use]
//! extern crate savefile_derive;
//! use std::fs::File;
//! use std::io::prelude::*;
//!
//!
//! #[derive(Serialize,Deserialize)]
//! struct Player {
//!		name : String,
//! 	strength : u32,
//!     inventory : Vec<String>,
//! }
//!
//! fn save(player:&Player) {
//! 	let mut f = File::create("save.bin").unwrap();
//!     let mut serializer = Serializer::new(&mut f, 0);
//!     player.serialize(&mut serializer);
//! }
//!
//! fn load() -> Player {
//! 	let mut f = File::open("save.bin").unwrap();
//!     let mut deserializer = Deserializer::new(&mut f, 0);
//!     Player::deserialize(&mut deserializer)
//! }
//!
//! fn main() {
//!     save(&Player { name: "Steve".to_string(), strength: 42,
//!         inventory: vec!(
//!             "wallet".to_string(),
//!             "car keys".to_string(),
//!             "glasses".to_string())});
//! 	assert_eq!(load().name,"Steve".to_string());
//!
//! }
//!
//! ```
//!

pub mod prelude;
mod savefile;
