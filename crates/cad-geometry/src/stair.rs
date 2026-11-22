//! Straight stairs: the data of `Shape::Stair` and its plan symbol. The stepped solid is in `cad-arch`.
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// A straight flight on a level, from the front of the first step at `a` (centre of the width) to the last riser at
/// `b`, which leads onto the floor `height` above. `risers` steps of equal height; the treads, one fewer, share the run
/// equally. Millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stair {
    pub a: DVec2,
    pub b: DVec2,
    pub width: f64,
    pub height: f64,
    pub risers: u32,
}

impl Stair {
    /// Risers needed to climb `height` with none taller than `max_riser`, at least 2.
    pub fn risers_for(height: f64, max_riser: f64) -> u32 {
        // The margin keeps an exact multiple (2880 / 180) from rounding up.
        (height / max_riser - 1e-9).ceil().max(2.0) as u32
    }

    /// Depth of one tread: the run over the treads (one fewer than the risers).
    pub fn tread(&self) -> f64 {
        self.a.distance(self.b) / f64::from(self.risers.max(2) - 1)
    }

    /// Height of one riser.
    pub fn riser(&self) -> f64 {
        self.height / f64::from(self.risers.max(1))
    }

    /// The footprint, counter-clockwise: right side at `a`, right at `b`, left at `b`, left at `a`.
    pub fn corners(&self) -> [DVec2; 4] {
        let side = (self.b - self.a).normalize_or_zero().perp() * (self.width / 2.0);
        [self.a - side, self.b - side, self.b + side, self.a + side]
    }

    /// Plan symbol: the right and left sides, one line per riser from `a` to `b` (right to left), the walking line
    /// from `a` to `b` and the two strokes of its arrowhead at `b`, a quarter of the width long.
    pub fn symbol(&self) -> Vec<[DVec2; 2]> {
        let [r0, r1, l1, l0] = self.corners();
        let d = (self.b - self.a).normalize_or_zero();
        let (t, h) = (self.tread(), self.width / 4.0);
        let mut out = vec![[r0, r1], [l0, l1]];
        out.extend((0..self.risers).map(|k| {
            let s = d * (t * f64::from(k));
            [r0 + s, l0 + s]
        }));
        out.push([self.a, self.b]);
        out.push([self.b, self.b - d * h + d.perp() * (h / 2.0)]);
        out.push([self.b, self.b - d * h - d.perp() * (h / 2.0)]);
        out
    }
}

#[cfg(test)]
#[path = "stair_tests.rs"]
mod tests;
