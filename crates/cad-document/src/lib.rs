pub mod constraints;
mod dimlink;
pub mod document;
pub mod feature;
pub mod file;
mod group;
pub mod history;
pub mod linetype;
mod openings;
pub mod rebuild;
mod rebuild_blend;
mod rebuild_pattern;
mod rebuild_shell;
mod rebuild_sweep;
mod rebuild_walls;
mod refs;
pub mod sketch;
pub mod units;

#[cfg(test)]
mod construction_tests;
#[cfg(test)]
mod constraint_tests;
#[cfg(test)]
mod constraint_expr_tests;
#[cfg(test)]
mod dimlink_tests;
#[cfg(test)]
mod document_tests;
#[cfg(test)]
mod group_tests;
#[cfg(test)]
mod hatch_tests;
#[cfg(test)]
mod pattern_feature_tests;
#[cfg(test)]
mod shell_feature_tests;
#[cfg(test)]
mod sweep_feature_tests;

pub use cad_constraints::{Constraint, PointKind, PointRef, Solution, SolveError, Status};
pub use cad_modeler::{Body, BodyEdge, Bvh, BoolOp, Convexity, EdgeGeom, Plane3, Surface, ray_triangle};
pub use cad_params::{ParamError, ParamSet, Value};
pub use document::{Command, ConstraintId, DocError, Document, Entity, EntityId, FORMAT_VERSION, Layer, LayerId};
pub use feature::{Axis3, BODY_TOLERANCE, BodyOp, EDGE_PICK_TOLERANCE, Feature, FeatureId, FeatureKind, MirrorSource, RevolveAxis, Target};
pub use history::History;
pub use linetype::LineType;
pub use refs::{FaceKey, edge_keys};
pub use rebuild::{FeatureState, RebuildResult, Rebuilder, panic_message};
pub use sketch::{BasePlane, Level, PlaneDef, Sketch, SketchId, SketchPlanes};
pub use units::{Settings, Units};
