//! Loft through sections. Every section loop is turned the same way and parametrised by arc length from the point
//! where a common reference ray leaves it; all loops are then evaluated at the union of their own corner parameters,
//! so every ring has the same points and contains its own corners, and consecutive rings are joined by ruled quads.
use crate::cap::triangulate;
use crate::sweep::outline;
use crate::{Body, BodyFace, ModelError, Plane3, Surface, TriMesh};
use cad_geometry::profile::Profile;
use glam::{DVec2, DVec3};

pub struct Section<'a> {
    pub profile: &'a Profile,
    pub plane: &'a Plane3,
}

/// A section loop in 3D with the arc length `cum[j]` at `pts[j]` (`cum[m]` is the length) and the arc length of its start.
struct Loop3 {
    pts: Vec<DVec3>,
    cum: Vec<f64>,
    start: f64,
}

impl Loop3 {
    fn len(&self) -> f64 {
        self.cum[self.pts.len()]
    }

    /// Parameters 0..1 of the loop's own points.
    fn corners(&self) -> Vec<f64> {
        let u = |c: f64| ((c - self.start) / self.len()).rem_euclid(1.0);
        self.cum[..self.pts.len()].iter().map(|&c| u(c)).map(|u| if u > 1.0 - 1e-9 { 0.0 } else { u }).collect()
    }

    fn at(&self, u: f64) -> DVec3 {
        let m = self.pts.len();
        let s = (self.start + u * self.len()).rem_euclid(self.len());
        let j = (self.cum.partition_point(|&c| c <= s).max(1) - 1).min(m - 1);
        let (a, b) = (self.pts[j], self.pts[(j + 1) % m]);
        a.lerp(b, ((s - self.cum[j]) / (self.cum[j + 1] - self.cum[j])).clamp(0.0, 1.0))
    }
}

/// The section's loop, turning counter-clockwise seen against `toward`, started where the ray from its centroid
/// along `reference` (projected into the section plane) leaves it.
fn prepare(sec: &Section, toward: DVec3, reference: DVec3, tol: f64) -> Result<Loop3, ModelError> {
    let plane = sec.plane;
    let mut pts2: Vec<DVec2> = outline(sec.profile, tol, false)[0].iter().map(|s| s.0).collect();
    let m = pts2.len();
    let turn: DVec3 = (0..m).map(|j| plane.point(pts2[j]).cross(plane.point(pts2[(j + 1) % m]))).sum();
    if turn.dot(toward) < 0.0 {
        pts2.reverse();
    }
    let flat = reference - plane.normal() * reference.dot(plane.normal());
    let r = if flat.length() < 1e-6 { DVec2::X } else { DVec2::new(flat.dot(plane.x), flat.dot(plane.y)).normalize() };
    let c = sec.profile.centroid();
    let pts: Vec<DVec3> = pts2.iter().map(|&p| plane.point(p)).collect();
    let mut cum = vec![0.0];
    cum.extend((0..m).scan(0.0, |acc, j| {
        *acc += pts[j].distance(pts[(j + 1) % m]);
        Some(*acc)
    }));
    // Nearest crossing of the ray c + t r with an edge p + s e.
    let mut best: Option<(f64, f64)> = None;
    for j in 0..m {
        let (p, e) = (pts2[j], pts2[(j + 1) % m] - pts2[j]);
        let den = r.perp_dot(e);
        if den.abs() < 1e-12 {
            continue;
        }
        let (t, s) = ((p - c).perp_dot(e) / den, (p - c).perp_dot(r) / den);
        if t > 1e-12 && (0.0..1.0).contains(&s) && best.is_none_or(|(bt, _)| t < bt) {
            best = Some((t, cum[j] + s * (cum[j + 1] - cum[j])));
        }
    }
    let (_, start) = best.ok_or(ModelError::Invalid("loft section has no start point"))?;
    Ok(Loop3 { pts, cum, start })
}

// shortcut: ruled between sections and no self-intersection check; add smooth interpolation and the check with NURBS surfaces
pub fn loft(sections: &[Section], tol: f64) -> Result<Body, ModelError> {
    if sections.len() < 2 {
        return Err(ModelError::Invalid("loft needs at least two sections"));
    }
    if sections.iter().any(|s| !s.profile.holes.is_empty()) {
        return Err(ModelError::Invalid("loft sections must be single loops without holes"));
    }
    if !tol.is_finite() || tol < 1e-6 {
        return Err(ModelError::Invalid("tessellation tolerance too small"));
    }
    let centroids: Vec<DVec3> = sections.iter().map(|s| s.plane.point(s.profile.centroid())).collect();
    if centroids.windows(2).any(|w| w[0].distance(w[1]) < 1e-6) {
        return Err(ModelError::Invalid("loft sections coincide"));
    }
    let k = sections.len();
    let toward = |i: usize| if i + 1 < k { centroids[i + 1] - centroids[i] } else { centroids[i] - centroids[i - 1] };
    let reference = sections[0].plane.x;
    let loops = (0..k).map(|i| prepare(&sections[i], toward(i), reference, tol)).collect::<Result<Vec<_>, _>>()?;

    let mut params: Vec<f64> = loops.iter().flat_map(Loop3::corners).collect();
    params.sort_by(f64::total_cmp);
    let mut shared: Vec<f64> = Vec::new();
    for p in params {
        if shared.last().is_none_or(|&l| p - l > 1e-9) {
            shared.push(p);
        }
    }
    let m = shared.len();
    let positions: Vec<DVec3> = loops.iter().flat_map(|l| shared.iter().map(|&u| l.at(u))).collect();
    let vid = |r: usize, j: usize| (r * m + j) as u32;
    let (mut indices, mut tri_face) = (Vec::new(), Vec::new());
    for i in 0..k - 1 {
        for j in 0..m {
            let (a, b, c, d) = (vid(i, j), vid(i, (j + 1) % m), vid(i + 1, (j + 1) % m), vid(i + 1, j));
            for t in [[a, b, c], [a, c, d]] {
                indices.extend(t);
                tri_face.push(i as u32);
            }
        }
    }
    let mut faces = vec![BodyFace { surface: Surface::Faceted, outward: true }; k - 1];
    let pos = |i: u32| positions[i as usize];
    for (ring, section_dir) in [(0, -toward(0)), (k - 1, toward(k - 1))] {
        let plane = sections[ring].plane;
        let n = plane.normal();
        let out = if n.dot(section_dir) < 0.0 { -n } else { n };
        let flat: Vec<DVec2> = (0..m)
            .map(|j| DVec2::new((pos(vid(ring, j)) - plane.origin).dot(plane.x), (pos(vid(ring, j)) - plane.origin).dot(plane.y)))
            .collect();
        let slot = faces.len() as u32;
        faces.push(BodyFace { surface: Surface::Plane { origin: pos(vid(ring, 0)), normal: out }, outward: true });
        for t in triangulate(&[flat])? {
            let [a, b, c] = t.map(|j| vid(ring, j));
            let facing = (pos(b) - pos(a)).cross(pos(c) - pos(a)).dot(out);
            indices.extend(if facing < 0.0 { [a, c, b] } else { [a, b, c] });
            tri_face.push(slot);
        }
    }
    let mesh = TriMesh { positions, indices, normals: Vec::new() };
    if !mesh.is_closed() || mesh.volume() <= 0.0 {
        return Err(ModelError::Invalid("loft did not produce a closed solid"));
    }
    let mut body = Body { mesh, tri_face, faces };
    body.merge_faces();
    body.recompute_normals();
    Ok(body)
}

#[cfg(test)]
#[path = "loft_tests.rs"]
mod tests;
