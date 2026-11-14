//! itSnows simulation core: pure logic, no OS or graphics dependencies.
//! Feed it per-frame inputs, get back a draw list.

pub mod blower;
pub mod flakes;
pub mod occlusion;
pub mod pile;
pub mod prim;
pub mod rng;
pub mod surfaces;
pub mod world;

pub use flakes::{Flake, Layer};
pub use occlusion::{window_top_surfaces, WinRect, MIN_SEGMENT};
pub use pile::{HeightMap, COL_W};
pub use prim::{Prim, Rgba};
pub use rng::Rng;
pub use world::*;
