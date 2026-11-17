//! Door or window in a wall: the data of `Shape::Opening`. Cuts, plan gaps and symbols are in `cad-arch`.
use crate::wall::Wall;
use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpeningKind {
    Door,
    Window,
}

/// A door or window hosted by a wall on the same level. Millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Opening {
    /// Entity id of the host wall.
    pub host: u64,
    /// Copy of the host wall, kept equal to it by the document after every edit; it places the opening for bounds,
    /// picking and drawing without a lookup.
    pub wall: Wall,
    /// Distance along the baseline from `wall.a` to the opening's centre.
    pub at: f64,
    pub width: f64,
    pub height: f64,
    /// Bottom of the opening above the level floor; 0 for a door.
    pub sill: f64,
    pub kind: OpeningKind,
    /// Door: the hinges are on the jamb toward the wall's `b` end, else toward `a`.
    pub hinge_b: bool,
    /// Door: the leaf opens to the left of the baseline `a` -> `b`, else to the right.
    pub swing_left: bool,
}

impl Opening {
    /// Start and end along the baseline from `wall.a`: `width` centred on `at`, slid back inside the wall when it
    /// sticks out; an opening wider than the wall takes all of it.
    pub fn span(&self) -> (f64, f64) {
        let len = self.wall.a.distance(self.wall.b);
        let half = (self.width / 2.0).min(len / 2.0);
        let c = self.at.clamp(half, len - half);
        (c - half, c + half)
    }

    /// The gap in plan, counter-clockwise like `Wall::corners`: right face at the start, right face at the end, left
    /// face at the end, left face at the start.
    pub fn corners(&self) -> [DVec2; 4] {
        let w = &self.wall;
        let d = (w.b - w.a).normalize_or_zero();
        let (l, r) = w.faces();
        let (u0, u1) = self.span();
        let p = |u: f64, s: f64| w.a + d * u + d.perp() * s;
        [p(u0, r), p(u1, r), p(u1, l), p(u0, l)]
    }
}

#[cfg(test)]
#[path = "opening_tests.rs"]
mod tests;
