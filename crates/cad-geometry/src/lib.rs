pub mod annot;
pub mod curve;
pub mod edit;
pub mod mesh;
pub mod opening;
pub mod pick;
pub mod profile;
pub mod room;
pub mod shape;
pub mod snap;
pub mod tol;
pub mod wall;
pub mod xform;

#[cfg(test)]
mod curve_edit_tests;
#[cfg(test)]
mod curve_shape_tests;

pub use glam::{DAffine2, DAffine3, DQuat, DVec2, DVec3};
pub use mesh::{Mesh, outline, outline_paths, tessellate};
pub use opening::{Opening, OpeningKind};
pub use pick::{distance_2d, point_segment_distance, ray_hit};
pub use room::{Room, Slab};
pub use shape::{GeomError, HatchPattern, Shape};
pub use tol::Tolerance;
pub use wall::{Wall, WallAlign};
