pub mod bom;
pub mod component;
pub mod constraints;
mod dimlink;
pub mod document;
pub mod feature;
pub mod fea_mesh;
pub mod file;
mod group;
pub mod history;
pub mod joint;
pub mod joint_loop;
pub mod linetype;
pub mod mass;
pub mod motion;
mod openings;
pub mod part;
pub mod rebuild;
mod rebuild_blend;
mod rebuild_draft;
mod rebuild_pattern;
mod rebuild_shell;
mod rebuild_sweep;
mod rebuild_walls;
mod refs;
pub mod samples;
pub mod sketch;
pub mod study;
pub mod units;
pub mod validation;

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
mod part_feature_tests;
#[cfg(test)]
mod pattern_feature_tests;
#[cfg(test)]
mod shell_feature_tests;
#[cfg(test)]
mod sweep_feature_tests;

pub use cad_constraints::{Constraint, PointKind, PointRef, Solution, SolveError, Status};
pub use cad_modeler::{Body, BodyEdge, Bvh, BoolOp, Convexity, EdgeGeom, Plane3, Surface, ray_triangle};
pub use cad_params::{ParamError, ParamSet, Value};
pub use document::{Command, ConstraintId, DocError, Document, Entity, EntityId, FORMAT_VERSION, Layer, LayerId, NextIds};
pub use feature::{Axis3, BODY_TOLERANCE, BodyOp, EDGE_PICK_TOLERANCE, Feature, FeatureId, FeatureKind, MirrorSource, RevolveAxis, Target};
pub use history::History;
pub use joint::{ExplodeStep, Frame, Joint, JointId, JointKind};
pub use linetype::LineType;
pub use cad_mech::{Check, Clearance, MATERIALS, Material, Meeting, Overlap, Part};
pub use bom::{BomRow, bom_csv};
pub use cad_analysis::{Accuracy, FEA, FOUR_BAR, FeaResult, MAX_DOFS, MESHED, MassProps, MeshStats, SAMPLED, SOLVED, TET_FACES, TetMesh, four_bar_rocker};
pub use study::{Load, LoadKind, Study, StudyReport};
pub use validation::{Benchmark, CANTILEVER, CANTILEVER_SIZES, SOLID_BOX, SOLID_CYLINDER, TENSION_BAR, benchmarks, validation_markdown};
pub use fea_mesh::MeshReport;
pub use mass::{MassReport, MassRow};
pub use motion::{Collision, MAX_STEPS, Motion, MotionStudy};
pub use component::{Component, ComponentId, mm};
pub use part::PartDef;
pub use refs::{FaceKey, FaceRef, edge_keys};
pub use rebuild::{FeatureState, RebuildResult, Rebuilder, panic_message};
pub use sketch::{BasePlane, Level, PlaneDef, Sketch, SketchId, SketchPlanes};
pub use units::{Settings, Units};
