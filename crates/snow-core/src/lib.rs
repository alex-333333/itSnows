//! itSnows simulation core: pure logic, no OS or graphics dependencies.
//! Feed it per-frame inputs, get back a draw list.

pub mod flakes;
pub mod prim;
pub mod rng;
pub mod world;

pub use flakes::{Flake, Layer};
pub use prim::{Prim, Rgba};
pub use rng::Rng;
pub use world::*;
