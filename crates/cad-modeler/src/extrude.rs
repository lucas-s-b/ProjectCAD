use crate::{ModelError, Plane3};
use cad_geometry::profile::{Loop, Profile, Seg};
use cad_topology::{Brep, CoEdge, Curve, Edge, Face, Surface, TOL};
use glam::DVec3;

/// The profile's loops as one list: segment k starts at profile vertex k and ends at `nxt[k]`;
/// `ranges` are the index ranges of the loops (outer first).
pub(crate) struct Flat<'a> {
    pub segs: Vec<&'a Seg>,
    pub nxt: Vec<usize>,
    pub ranges: Vec<(usize, usize)>,
}

pub(crate) fn flatten(profile: &Profile) -> Flat<'_> {
    let loops: Vec<&Loop> = std::iter::once(&profile.outer).chain(&profile.holes).collect();
    let (mut nxt, mut ranges, mut at) = (Vec::new(), Vec::new(), 0);
    for l in &loops {
        ranges.push((at, at + l.len()));
        nxt.extend((0..l.len()).map(|i| at + (i + 1) % l.len()));
        at += l.len();
    }
    Flat { segs: loops.iter().flat_map(|l| l.iter()).collect(), nxt, ranges }
}

/// Edge curve of a profile segment whose arc centre is `center` in 3D and whose plane normal is `normal`.
pub(crate) fn seg_curve(seg: &Seg, center: DVec3, normal: DVec3) -> Curve {
    match seg {
        Seg::Line { .. } => Curve::Line,
        Seg::Arc { sweep, .. } => Curve::Arc { center, normal: if *sweep > 0.0 { normal } else { -normal }, sweep: sweep.abs() },
    }
}

/// Loops of a cap whose edges start at `first_edge`. Seen from outside a cap must run counter-clockwise:
/// the profile's own direction when the outside is on the side of the plane normal.
pub(crate) fn cap_loops(ranges: &[(usize, usize)], first_edge: usize, outward: bool) -> Vec<Vec<CoEdge>> {
    let coedge = |k: usize, reversed| CoEdge { edge: (first_edge + k) as u32, reversed };
    ranges
        .iter()
        .map(|&(a, b)| if outward { (a..b).map(|k| coedge(k, false)).collect() } else { (a..b).rev().map(|k| coedge(k, true)).collect() })
        .collect()
}

/// Extrudes the profile from the plane by `distance` along the plane normal (negative: against it).
pub fn extrude(profile: &Profile, plane: &Plane3, distance: f64) -> Result<Brep, ModelError> {
    if !distance.is_finite() || distance.abs() <= TOL {
        return Err(ModelError::Invalid("extrusion distance is zero"));
    }
    let n = plane.normal();
    let up = n * distance;
    let Flat { segs, nxt, ranges } = flatten(profile);
    let total = segs.len();
    let base: Vec<_> = segs.iter().map(|s| plane.point(s.start())).collect();

    let mut vertices = base.clone();
    vertices.extend(base.iter().map(|p| *p + up));
    // Edge k: bottom of segment k; total + k: top; 2 * total + k: vertical at vertex k.
    let mut edges = Vec::with_capacity(3 * total);
    for (shift, lift) in [(0, DVec3::ZERO), (total, up)] {
        for (k, s) in segs.iter().enumerate() {
            let center = if let Seg::Arc { center, .. } = s { plane.point(*center) + lift } else { DVec3::ZERO };
            edges.push(Edge { v: [(shift + k) as u32, (shift + nxt[k]) as u32], curve: seg_curve(s, center, n) });
        }
    }
    edges.extend((0..total).map(|k| Edge { v: [k as u32, (total + k) as u32], curve: Curve::Line }));

    let mut faces = Vec::with_capacity(total + 2);
    for (shift, outward) in [(0, distance < 0.0), (total, distance > 0.0)] {
        let origin = if shift == 0 { plane.origin } else { plane.origin + up };
        faces.push(Face { surface: Surface::Plane { origin, normal: n }, outward, loops: cap_loops(&ranges, shift, outward) });
    }
    for (k, s) in segs.iter().enumerate() {
        let (p, q) = (k as u32, nxt[k] as u32);
        let (surface, outward) = match s {
            Seg::Line { .. } => {
                let t = base[nxt[k]] - base[k];
                (Surface::Plane { origin: base[k], normal: t.cross(n).normalize() }, true)
            }
            // The region lies to the left of the travel: toward the centre for a counter-clockwise arc.
            Seg::Arc { center, radius, sweep, .. } => {
                (Surface::Cylinder { origin: plane.point(*center), axis: n, radius: *radius }, *sweep > 0.0)
            }
        };
        let (bottom, top, vp, vq) = (k as u32, (total + k) as u32, (2 * total) as u32 + p, (2 * total) as u32 + q);
        let c = |edge, reversed| CoEdge { edge, reversed };
        // Bottom, up at q, back along the top, down at p is counter-clockwise seen from outside when going up.
        let ring = if distance > 0.0 {
            vec![c(bottom, false), c(vq, false), c(top, true), c(vp, true)]
        } else {
            vec![c(vp, false), c(top, false), c(vq, true), c(bottom, true)]
        };
        faces.push(Face { surface, outward, loops: vec![ring] });
    }
    let brep = Brep { vertices, edges, faces };
    brep.validate()?;
    Ok(brep)
}
