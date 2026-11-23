//! Engineering analysis on solids: mass properties (Phase 6). Every result carries an `Accuracy` label. Works on
//! meshes and numbers, knows no document.
mod accuracy;
mod mass;

pub use accuracy::{Accuracy, TESSELLATION};
pub use mass::MassProps;
