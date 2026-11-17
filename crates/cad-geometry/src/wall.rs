//! Straight wall: the data of `Shape::Wall`. Joins, plan and solids are in `cad-arch`.
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// Which side of the baseline the wall lies on, looking from `a` to `b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallAlign {
    /// The baseline runs in the middle of the wall.
    Center,
    /// The wall lies on the left of the baseline; the baseline is its right face.
    Left,
    /// The wall lies on the right of the baseline; the baseline is its left face.
    Right,
}

/// A straight wall on a level: baseline `a` to `b` in the level's sketch coordinates, millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    pub a: DVec2,
    pub b: DVec2,
    pub thickness: f64,
    /// From the level's floor up.
    pub height: f64,
    pub align: WallAlign,
}

impl Wall {
    /// Offsets of the left and right faces along the left normal of a -> b: Center (t/2, -t/2), Left (t, 0), Right (0, -t).
    pub fn faces(&self) -> (f64, f64) {
        let t = self.thickness;
        match self.align {
            WallAlign::Center => (t / 2.0, -t / 2.0),
            WallAlign::Left => (t, 0.0),
            WallAlign::Right => (0.0, -t),
        }
    }

    /// The outline without joins, counter-clockwise: right face at `a`, right face at `b`, left face at `b`, left face at `a`.
    pub fn corners(&self) -> [DVec2; 4] {
        let n = (self.b - self.a).normalize_or_zero().perp();
        let (l, r) = self.faces();
        [self.a + n * r, self.b + n * r, self.b + n * l, self.a + n * l]
    }
}

#[cfg(test)]
#[path = "wall_tests.rs"]
mod tests;
