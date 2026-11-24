//! Sweep of a profile along a path. The profile outline is carried along the path by projection. With path points
//! `P_0..P_n` and unit directions `d_i` (from `P_i` to `P_i+1`), ring 0 is the outline in its plane and ring `i+1` is
//! ring `i` moved along `d_i` onto the cut plane at `P_i+1`: the bisector plane of two segments (mitred corners), or
//! perpendicular to the last direction at the end of an open path. A closed path starts with a mitre at `P_0` too.
use crate::cap::triangulate;
use crate::extrude::flatten;
use crate::{Body, BodyFace, ModelError, Plane3, Surface, TriMesh};
use cad_geometry::profile::{Profile, Seg};
use glam::{DVec2, DVec3};
use std::collections::HashMap;

const FOLDED: ModelError = ModelError::Invalid("sweep folds over itself: the profile is too large for the path curvature");
const TURNED: ModelError = ModelError::Invalid("sweep path turns back on itself");
const TWISTED: ModelError = ModelError::Invalid("closed sweep path twists: not supported");
const NOT_SOLID: ModelError = ModelError::Invalid("sweep did not produce a closed solid");

/// Consecutive pieces share their end point.
pub struct PathPiece {
    pub points: Vec<DVec3>,
    pub straight: bool,
}

/// Chords of an arc whose sagitta stays below `tol`.
pub(crate) fn arc_steps(radius: f64, sweep: f64, tol: f64) -> usize {
    let d = 2.0 * (1.0 - tol / radius).clamp(-1.0, 1.0).acos();
    ((sweep.abs() / d).ceil().clamp(2.0, 1e6) as usize).max(2)
}

/// Flattens a 2D path on a plane into 3D pieces: a Line is one straight piece; an Arc is one piece of chords.
pub fn path_pieces(path: &[Seg], plane: &Plane3, tol: f64) -> Vec<PathPiece> {
    let mut out: Vec<PathPiece> = Vec::new();
    for s in path {
        let (mut points, straight) = match s {
            Seg::Line { a, b } => (vec![plane.point(*a), plane.point(*b)], true),
            Seg::Arc { radius, sweep, .. } => {
                let n = arc_steps(*radius, *sweep, tol);
                ((0..=n).map(|k| plane.point(s.point(k as f64 / n as f64))).collect(), false)
            }
        };
        if let Some(last) = out.last().and_then(|p| p.points.last()) {
            points[0] = *last;
        }
        out.push(PathPiece { points, straight });
    }
    out
}

/// Every loop of the profile sampled once (a line by its start, an arc by its chords), paired with the index
/// (in `flatten` order) of the segment that starts at the sample. `flip` reverses every loop.
pub(crate) fn outline(profile: &Profile, tol: f64, flip: bool) -> Vec<Vec<(DVec2, usize)>> {
    let flat = flatten(profile);
    flat.ranges
        .iter()
        .map(|&(from, to)| {
            let mut lp = Vec::new();
            for (k, &s) in flat.segs.iter().enumerate().take(to).skip(from) {
                match s {
                    Seg::Line { a, .. } => lp.push((*a, k)),
                    Seg::Arc { radius, sweep, .. } => {
                        let n = arc_steps(*radius, *sweep, tol);
                        lp.extend((0..n).map(|j| (s.point(j as f64 / n as f64), k)));
                    }
                }
            }
            if flip {
                // The segment between reversed samples k and k+1 is the old segment that started at m-2-k.
                let m = lp.len();
                lp = (0..m).map(|k| (lp[m - 1 - k].0, lp[(2 * m - 2 - k) % m].1)).collect();
            }
            lp
        })
        .collect()
}

/// A unit direction halfway between `a` and `b`, the normal of their mitre plane.
fn bisector(a: DVec3, b: DVec3) -> Result<DVec3, ModelError> {
    (a + b).try_normalize().filter(|m| a.dot(*m).abs() >= 1e-6).ok_or(TURNED)
}

pub fn sweep(profile: &Profile, plane: &Plane3, path: &[PathPiece], closed: bool, tol: f64) -> Result<Body, ModelError> {
    if !tol.is_finite() || tol < 1e-6 {
        return Err(ModelError::Invalid("tessellation tolerance too small"));
    }
    let mut pts: Vec<DVec3> = path.iter().take(1).flat_map(|p| p.points.first().copied()).collect();
    let mut seg_piece = Vec::new();
    for (k, p) in path.iter().enumerate() {
        for q in p.points.iter().skip(1) {
            pts.push(*q);
            seg_piece.push(k);
        }
    }
    if pts.len() < 2 {
        return Err(ModelError::Invalid("sweep path is empty"));
    }
    let n = pts.len() - 1;
    if closed {
        if pts[n].distance(pts[0]) > 1e-6 {
            return Err(ModelError::Invalid("closed sweep path does not return to its start"));
        }
        pts[n] = pts[0];
    }
    let dirs = pts
        .windows(2)
        .map(|w| (w[1] - w[0]).try_normalize().filter(|_| w[0].distance(w[1]) > 1e-9))
        .collect::<Option<Vec<DVec3>>>()
        .ok_or(ModelError::Invalid("sweep path has a zero-length segment"))?;
    let normal = plane.normal();
    if normal.dot(dirs[0]).abs() < 0.1 {
        return Err(ModelError::Invalid("profile plane must not contain the path direction"));
    }
    let mut cut = vec![DVec3::ZERO; n + 1];
    for k in 1..n {
        cut[k] = bisector(dirs[k - 1], dirs[k])?;
    }
    cut[n] = if closed { bisector(dirs[n - 1], dirs[0])? } else { dirs[n - 1] };
    cut[0] = if closed { cut[n] } else { dirs[0] };

    let loops = outline(profile, tol, normal.dot(dirs[0]) < 0.0);
    let flat2d: Vec<DVec2> = loops.iter().flatten().map(|s| s.0).collect();
    let seg: Vec<usize> = loops.iter().flatten().map(|s| s.1).collect();
    let m = flat2d.len();
    let mut next = Vec::with_capacity(m);
    let mut at = 0;
    for l in &loops {
        next.extend((0..l.len()).map(|k| at + (k + 1) % l.len()));
        at += l.len();
    }

    let mut ring: Vec<DVec3> = flat2d.iter().map(|&q| plane.point(q)).collect();
    if closed {
        let (d, b) = (dirs[0], cut[0]);
        ring = ring.iter().map(|&q| q + d * ((pts[0] - q).dot(b) / d.dot(b))).collect();
    }
    let mut rings = vec![ring];
    for i in 0..n {
        let (d, b) = (dirs[i], cut[i + 1]);
        let mut moved = Vec::with_capacity(m);
        for &q in &rings[i] {
            let s = (pts[i + 1] - q).dot(b) / d.dot(b);
            if s <= 1e-9 {
                return Err(FOLDED);
            }
            moved.push(q + d * s);
        }
        rings.push(moved);
    }
    if closed {
        let last = rings.pop().unwrap_or_default();
        if last.iter().zip(&rings[0]).any(|(a, b)| a.distance(*b) > 1e-6) {
            return Err(TWISTED);
        }
    }

    let nr = rings.len();
    let vid = |r: usize, j: usize| (r * m + j) as u32;
    let positions: Vec<DVec3> = rings.concat();
    let (mut indices, mut tri_face) = (Vec::new(), Vec::new());
    // One face per (profile segment, path piece); `first` keeps the ring and first triangle of each.
    let mut slots: HashMap<(usize, usize), u32> = HashMap::new();
    let mut first: Vec<((usize, usize), usize, [u32; 3])> = Vec::new();
    for (i, &piece) in seg_piece.iter().enumerate() {
        let i1 = (i + 1) % nr;
        for j in 0..m {
            let (a, b, c, d) = (vid(i, j), vid(i, next[j]), vid(i1, next[j]), vid(i1, j));
            let key = (seg[j], piece);
            let slot = *slots.entry(key).or_insert_with(|| {
                first.push((key, i, [a, b, c]));
                first.len() as u32 - 1
            });
            for t in [[a, b, c], [a, c, d]] {
                indices.extend(t);
                tri_face.push(slot);
            }
        }
    }

    // A ring is the true profile shape (so a cylinder is exact) while no mitre has sheared it.
    let mut true_ring = vec![!closed && normal.cross(dirs[0]).length() < 1e-9; n];
    for i in 1..n {
        true_ring[i] = true_ring[i - 1] && dirs[i - 1].cross(dirs[i]).length() < 1e-9;
    }
    let segs = flatten(profile).segs;
    let pos = |i: u32| positions[i as usize];
    let mut faces: Vec<BodyFace> = first
        .iter()
        .map(|&((sg, pc), i, [a, b, c])| {
            let tri_normal = (pos(b) - pos(a)).cross(pos(c) - pos(a)).normalize_or_zero();
            let surface = match segs[sg] {
                Seg::Line { .. } if path[pc].straight && tri_normal != DVec3::ZERO => Surface::Plane { origin: pos(a), normal: tri_normal },
                Seg::Arc { center, radius, .. } if path[pc].straight && true_ring[i] => Surface::Cylinder {
                    origin: plane.point(*center) + rings[i][0] - rings[0][0],
                    axis: dirs[i],
                    radius: *radius,
                },
                _ => Surface::Faceted,
            };
            let outward = surface == Surface::Faceted || surface.normal_at(pos(a)).dot(tri_normal) > 0.0;
            BodyFace { surface, outward }
        })
        .collect();

    if !closed {
        let rings2d: Vec<Vec<DVec2>> = loops.iter().map(|l| l.iter().map(|s| s.0).collect()).collect();
        let tris = triangulate(&rings2d)?;
        let start_out = if normal.dot(dirs[0]) > 0.0 { -normal } else { normal };
        for (ring, out) in [(0, start_out), (n, dirs[n - 1])] {
            let slot = faces.len() as u32;
            faces.push(BodyFace { surface: Surface::Plane { origin: rings[ring][0], normal: out }, outward: true });
            for t in &tris {
                let [a, b, c] = t.map(|k| vid(ring, k));
                let facing = (pos(b) - pos(a)).cross(pos(c) - pos(a)).dot(out);
                indices.extend(if facing < 0.0 { [a, c, b] } else { [a, b, c] });
                tri_face.push(slot);
            }
        }
    }

    let mesh = TriMesh { positions, indices, normals: Vec::new() };
    if !mesh.is_closed() || mesh.volume() <= 0.0 {
        return Err(NOT_SOLID);
    }
    let mut body = Body { mesh, tri_face, faces };
    body.merge_faces();
    body.recompute_normals();
    Ok(body)
}

#[cfg(test)]
#[path = "sweep_tests.rs"]
mod tests;
