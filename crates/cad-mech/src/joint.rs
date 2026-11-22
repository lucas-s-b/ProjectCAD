//! Joint frame math: a frame from an origin and an axis, the mate of two frames, and the frame of a body face.
use cad_geometry::{DAffine3, DVec3};
use cad_modeler::{Body, Surface};

/// The frame at `origin` whose Z is `axis` normalised. X is world X with its part along the axis removed (world Y when
/// the axis is within about 26 degrees of X), Y = Z x X; axis +Z gives the world axes. None when `axis` is zero or not finite.
pub fn frame(origin: DVec3, axis: DVec3) -> Option<DAffine3> {
    let z = axis.try_normalize()?;
    let r = if z.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let x = (r - z * r.dot(z)).normalize();
    Some(DAffine3::from_cols(x, z.cross(x), z, origin))
}

/// Where a child goes relative to its parent: parent_frame x motion x flip x child_frame⁻¹. The child frame lands on
/// the parent frame turned half a turn about its X (the two axes point at each other, like the outward normals of two
/// touching faces); `motion` acts in parent-frame coordinates (a turn about Z or a move along Z).
pub fn mate(parent_frame: DAffine3, motion: DAffine3, child_frame: DAffine3) -> DAffine3 {
    // Half a turn about X, written exactly (from_rotation_x(PI) leaves 1e-16 terms).
    let flip = DAffine3::from_cols(DVec3::X, -DVec3::Y, -DVec3::Z, DVec3::ZERO);
    parent_frame * motion * flip * child_frame.inverse()
}

/// Origin and unit axis of face `face` of `body`: a plane gives the area centroid of its triangles and its outward
/// normal; a cylinder gives its axis direction and the point of the axis nearest that centroid. None for other
/// surfaces, a face index out of range or a face without triangles.
pub fn face_frame(body: &Body, face: usize) -> Option<(DVec3, DVec3)> {
    let f = body.faces.get(face)?;
    let (mut area, mut sum) = (0.0, DVec3::ZERO);
    for (&t, &tf) in body.mesh.indices.as_chunks::<3>().0.iter().zip(&body.tri_face) {
        if tf as usize == face {
            let [a, b, c] = t.map(|i| body.mesh.positions[i as usize]);
            let w = (b - a).cross(c - a).length();
            area += w;
            sum += (a + b + c) * (w / 3.0);
        }
    }
    if area <= 0.0 {
        return None;
    }
    let centroid = sum / area;
    match f.surface {
        Surface::Plane { normal, .. } => Some((centroid, normal.normalize() * if f.outward { 1.0 } else { -1.0 })),
        Surface::Cylinder { origin, axis, .. } => {
            let a = axis.normalize();
            Some((origin + a * a.dot(centroid - origin), a))
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "joint_tests.rs"]
mod tests;
