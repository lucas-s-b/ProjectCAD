//! Rooms and floor slabs: the data of `Shape::Room` and `Shape::Slab`. Room areas and slab solids are in `cad-arch`.
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// A room on a level: a point inside the area its walls enclose, with the room's name and number. The area itself is
/// found from the walls on every rebuild (`cad_arch::regions`), so name and number stay when walls move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Room {
    pub seed: DVec2,
    pub name: String,
    pub number: String,
}

/// A floor slab on a level: a closed outline in the level's sketch (either orientation), `thickness` thick below the
/// level's floor. Millimetres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slab {
    pub outline: Vec<DVec2>,
    pub thickness: f64,
}

impl Slab {
    /// Signed area of the outline, positive counter-clockwise.
    pub fn area(&self) -> f64 {
        let p = &self.outline;
        (0..p.len()).map(|k| p[k].perp_dot(p[(k + 1) % p.len()])).sum::<f64>() / 2.0
    }
}

#[cfg(test)]
#[path = "room_tests.rs"]
mod tests;
