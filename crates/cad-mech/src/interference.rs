//! Interference, contact and clearance between placed solids: common volumes from the manifold boolean, distances
//! from parry's triangle-mesh queries.
use cad_geometry::DVec3;
use cad_modeler::{Body, BoolOp, ModelError, boolean};
use parry3d_f64::math::{Isometry, Point};
use parry3d_f64::query::{contact, distance};
use parry3d_f64::shape::TriMesh;

/// Common volumes up to this (mm³) count as contact.
pub const CONTACT_VOLUME: f64 = 1e-6;
/// Surfaces at most this far apart (mm) touch.
pub const CONTACT_GAP: f64 = 1e-6;

/// How two solids meet.
#[derive(Debug, Clone, PartialEq)]
pub enum Overlap {
    /// They share a volume: its size (mm³) and the common solid.
    Interference { volume: f64, region: Body },
    /// They touch, or share only a sliver thinner than the facet tolerance (a bolt in its nut: threads are cosmetic).
    Contact,
}

/// Distance between the surfaces of two solids and the closest points (on the first, on the second); no points when
/// the surfaces cross (distance 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clearance {
    pub distance: f64,
    pub points: Option<(DVec3, DVec3)>,
}

/// A pair of solids that overlap or touch, by the ids given to `check`; Err when the boolean failed.
#[derive(Debug, Clone, PartialEq)]
pub struct Meeting {
    pub a: u64,
    pub b: u64,
    pub kind: Result<Overlap, String>,
}

/// The result of `check`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Check {
    /// Pairs that overlap or touch, in pair order (a before b as given).
    pub meetings: Vec<Meeting>,
    /// The other pairs whose surfaces are within the limit: (a, b, clearance), nearest first.
    pub clearances: Vec<(u64, u64, Clearance)>,
}

/// The boxes (lo, hi) are at most `gap` apart along every axis.
fn near((alo, ahi): (DVec3, DVec3), (blo, bhi): (DVec3, DVec3), gap: f64) -> bool {
    (0..3).all(|k| alo[k] <= bhi[k] + gap && blo[k] <= ahi[k] + gap)
}

/// The common solid of `a` and `b`; None when they share no volume (an empty or not closed intersection, which is
/// what touching faces give).
fn common(a: &Body, b: &Body) -> Result<Option<Body>, ModelError> {
    match boolean(a, b, BoolOp::Intersection) {
        Ok(r) => Ok(Some(r)),
        Err(ModelError::Invalid("boolean result is empty" | "boolean result is not a closed solid")) => Ok(None),
        Err(e) => Err(e),
    }
}

/// How `a` and `b` meet; None when they are apart. A common solid counts as contact when its volume is at most
/// `CONTACT_VOLUME` or its mean thickness (2 x volume / area) at most `thickness`, so faceted copies of one cylinder
/// (a cosmetic thread in its nut) touch; no common solid with surfaces within `CONTACT_GAP` is contact too.
pub fn overlap(a: &Body, b: &Body, thickness: f64) -> Result<Option<Overlap>, ModelError> {
    if !near(a.bbox(), b.bbox(), CONTACT_GAP) {
        return Ok(None);
    }
    meet(a, b, thickness, clearance(a, b, CONTACT_GAP).is_some())
}

/// `overlap` once the boxes are near; `touching` says whether the surfaces are within `CONTACT_GAP`.
fn meet(a: &Body, b: &Body, thickness: f64, touching: bool) -> Result<Option<Overlap>, ModelError> {
    if let Some(region) = common(a, b)? {
        let volume = region.volume();
        if volume > CONTACT_VOLUME && 2.0 * volume / region.area() > thickness {
            return Ok(Some(Overlap::Interference { volume, region }));
        }
        return Ok(Some(Overlap::Contact));
    }
    Ok(touching.then_some(Overlap::Contact))
}

/// The triangles of `b` as a parry mesh; None for an empty mesh.
fn trimesh(b: &Body) -> Option<TriMesh> {
    let points = b.mesh.positions.iter().map(|p| Point::new(p.x, p.y, p.z)).collect();
    let tris = b.mesh.indices.as_chunks::<3>().0.to_vec();
    TriMesh::new(points, tris).ok()
}

/// Distance and closest points of two parry meshes when at most `limit` apart, else None.
fn mesh_clearance(ma: &TriMesh, mb: &TriMesh, limit: f64) -> Option<Clearance> {
    let at = Isometry::identity();
    let v = |p: Point<f64>| DVec3::new(p.x, p.y, p.z);
    // Best-first distance first: cheap, and it bounds the closest-point search below.
    let d = distance(&at, ma, &at, mb).ok()?;
    if d > limit {
        return None;
    }
    if d <= 0.0 {
        return Some(Clearance { distance: 0.0, points: None });
    }
    // Closest points from `contact`, not `closest_points`: parry 0.19's mesh-mesh `closest_points` panics ("composite
    // shape must not be empty") when a visited triangle has nothing within the margin. A prediction just above the
    // known distance keeps the search to the triangles near the closest pair.
    match contact(&at, ma, &at, mb, d * (1.0 + 1e-6) + 1e-9).ok().flatten() {
        Some(c) => Some(Clearance { distance: c.dist.max(0.0), points: Some((v(c.point1), v(c.point2))) }),
        None => Some(Clearance { distance: d, points: None }),
    }
}

/// Distance and closest points of the surfaces of `a` and `b` when at most `limit` apart (parry), else None. Only
/// surfaces count: a solid wholly inside another is reported by `overlap`, not here.
pub fn clearance(a: &Body, b: &Body, limit: f64) -> Option<Clearance> {
    mesh_clearance(&trimesh(a)?, &trimesh(b)?, limit)
}

/// Box `inner` lies inside box `outer`.
fn inside((ilo, ihi): (DVec3, DVec3), (olo, ohi): (DVec3, DVec3)) -> bool {
    (0..3).all(|k| olo[k] <= ilo[k] && ihi[k] <= ohi[k])
}

/// Every pair of `solids` (id, solid) whose boxes are within `limit`: overlapping or touching pairs (`overlap` with
/// `thickness`) go to `meetings`, the others whose surfaces are at most `limit` apart to `clearances`.
pub fn check(solids: &[(u64, &Body)], thickness: f64, limit: f64) -> Check {
    let boxes: Vec<(DVec3, DVec3)> = solids.iter().map(|(_, b)| b.bbox()).collect();
    let meshes: Vec<Option<TriMesh>> = solids.iter().map(|(_, b)| trimesh(b)).collect();
    let mut out = Check::default();
    for i in 0..solids.len() {
        for j in i + 1..solids.len() {
            let ((ia, a), (ib, b)) = (solids[i], solids[j]);
            if !near(boxes[i], boxes[j], limit.max(CONTACT_GAP)) {
                continue;
            }
            let c = match (&meshes[i], &meshes[j]) {
                (Some(x), Some(y)) => mesh_clearance(x, y, limit.max(CONTACT_GAP)),
                _ => None,
            };
            let touching = c.is_some_and(|c| c.distance <= CONTACT_GAP);
            let within = c.filter(|c| c.distance <= limit).map(|c| (ia, ib, c));
            // Surfaces apart and neither box inside the other: no common volume, so the boolean is skipped.
            if !touching && !inside(boxes[i], boxes[j]) && !inside(boxes[j], boxes[i]) {
                out.clearances.extend(within);
                continue;
            }
            match meet(a, b, thickness, touching) {
                Ok(Some(o)) => out.meetings.push(Meeting { a: ia, b: ib, kind: Ok(o) }),
                Err(e) => out.meetings.push(Meeting { a: ia, b: ib, kind: Err(e.to_string()) }),
                Ok(None) => out.clearances.extend(within),
            }
        }
    }
    // Stable sort: equal distances keep the pair order.
    out.clearances.sort_by(|x, y| x.2.distance.total_cmp(&y.2.distance));
    out
}

#[cfg(test)]
#[path = "interference_tests.rs"]
mod tests;
