//! Engineering analysis on solids: mass properties, the four-bar position equation and tetrahedral meshes for finite elements (Phase 6). Every result carries an `Accuracy` label. Works on meshes and numbers, knows no document.
mod accuracy;
mod four_bar;
mod mass;
mod tet_mesh;

pub use accuracy::{Accuracy, MESHED, SAMPLED, SOLVED, TESSELLATION};
pub use four_bar::{FOUR_BAR, four_bar_rocker};
pub use mass::MassProps;
pub use tet_mesh::{KUHN, MAX_CELLS, MeshStats, TET_FACES, TET10_EDGES, TetMesh, min_dihedral_angle, radius_ratio, tet_mesh, tet_volume};
