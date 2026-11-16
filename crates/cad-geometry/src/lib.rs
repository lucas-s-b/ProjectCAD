pub mod annot;
pub mod curve;
pub mod edit;
pub mod mesh;
pub mod pick;
pub mod profile;
pub mod shape;
pub mod snap;
pub mod tol;
pub mod xform;

#[cfg(test)]
mod curve_shape_tests;

pub use glam::{DAffine2, DAffine3, DQuat, DVec2, DVec3};
pub use mesh::{Mesh, outline, outline_paths, tessellate};
pub use pick::{distance_2d, point_segment_distance, ray_hit};
pub use shape::{GeomError, Shape};
pub use tol::Tolerance;
