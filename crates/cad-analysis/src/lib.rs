//! Engineering analysis on solids: mass properties and the four-bar position equation (Phase 6). Every result carries an `Accuracy` label. Works on meshes and numbers, knows no document.
mod accuracy;
mod four_bar;
mod mass;

pub use accuracy::{Accuracy, SAMPLED, SOLVED, TESSELLATION};
pub use four_bar::{FOUR_BAR, four_bar_rocker};
pub use mass::MassProps;
