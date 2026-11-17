mod blend;
mod body;
mod bvh;
mod boolean;
mod cap;
mod edges;
mod extrude;
#[cfg(test)]
mod fixtures;
mod loft;
mod mass;
mod plane;
mod revolve;
mod shell;
mod sweep;
mod tess;

pub use blend::{chamfer_edge, fillet_edge};
pub use body::{Body, BodyFace, union_all};
pub use bvh::{Bvh, ray_triangle};
pub use boolean::{BoolOp, boolean};
pub use edges::{BodyEdge, Convexity, EdgeGeom};
pub use cad_geometry::shape::GeomError;
pub use cad_topology::{Surface, TopoError};
pub use extrude::extrude;
pub use loft::{Section, loft};
pub use mass::prism_volume;
pub use plane::Plane3;
pub use revolve::revolve;
pub use shell::shell;
pub use sweep::{PathPiece, path_pieces, sweep};
pub use tess::{TriMesh, tessellate, tessellate_with_faces};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ModelError {
    #[error(transparent)]
    Geometry(#[from] GeomError),
    #[error(transparent)]
    Topology(#[from] TopoError),
    #[error("{0}")]
    Invalid(&'static str),
}

#[cfg(test)]
mod extrude_tests;
#[cfg(test)]
mod revolve_tests;
#[cfg(test)]
mod tess_tests;
