use glam::{DAffine3, DQuat, DVec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Curve {
    /// Straight from v[0] to v[1].
    Line,
    /// From v[0], counter-clockwise about `normal` by `sweep` (0 < sweep <= PI), ends at v[1].
    Arc { center: DVec3, normal: DVec3, sweep: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Surface {
    Plane { origin: DVec3, normal: DVec3 },
    Cylinder { origin: DVec3, axis: DVec3, radius: f64 },
    /// `axis` points from the apex into the cone.
    Cone { apex: DVec3, axis: DVec3, half_angle: f64 },
    Sphere { center: DVec3, radius: f64 },
    Torus { center: DVec3, axis: DVec3, major: f64, minor: f64 },
    /// A free-form surface that is known only through its facets.
    Faceted,
}

impl Curve {
    /// Point at `t` in 0..=1 along the edge from `a` to `b`.
    pub fn point(&self, a: DVec3, b: DVec3, t: f64) -> DVec3 {
        if t == 0.0 {
            return a;
        }
        if t == 1.0 {
            return b;
        }
        match self {
            Curve::Line => a.lerp(b, t),
            Curve::Arc { center, normal, sweep } => *center + DQuat::from_axis_angle(*normal, sweep * t) * (a - *center),
        }
    }
}

/// `v` normalised, or `fallback` when `v` has no direction.
fn unit_or(v: DVec3, fallback: DVec3) -> DVec3 {
    if v.length() > 1e-12 { v.normalize() } else { fallback }
}

/// Splits `d` into the part along unit `axis` and the unit direction away from the axis.
fn axial(d: DVec3, axis: DVec3) -> (f64, f64, DVec3) {
    let h = d.dot(axis);
    let radial = d - axis * h;
    (h, radial.length(), unit_or(radial, axis.any_orthonormal_vector()))
}

impl Surface {
    pub(crate) fn is_finite(&self) -> bool {
        match self {
            Surface::Plane { origin, normal } => origin.is_finite() && normal.is_finite(),
            Surface::Cylinder { origin, axis, radius } => origin.is_finite() && axis.is_finite() && radius.is_finite(),
            Surface::Cone { apex, axis, half_angle } => apex.is_finite() && axis.is_finite() && half_angle.is_finite(),
            Surface::Sphere { center, radius } => center.is_finite() && radius.is_finite(),
            Surface::Torus { center, axis, major, minor } => {
                center.is_finite() && axis.is_finite() && major.is_finite() && minor.is_finite()
            }
            Surface::Faceted => true,
        }
    }

    /// Closest point on the surface.
    pub fn project(&self, p: DVec3) -> DVec3 {
        match self {
            Surface::Plane { origin, normal } => p - *normal * (p - *origin).dot(*normal),
            Surface::Cylinder { origin, axis, radius } => {
                let (h, _, r) = axial(p - *origin, *axis);
                *origin + *axis * h + r * *radius
            }
            Surface::Cone { apex, axis, half_angle } => {
                let (h, rho, r) = axial(p - *apex, *axis);
                let (sin, cos) = half_angle.sin_cos();
                // Distance along the generator; behind the apex or on the axis the apex is returned.
                let s = h * cos + rho * sin;
                if s <= 0.0 || rho < 1e-12 { *apex } else { *apex + (*axis * cos + r * sin) * s }
            }
            Surface::Sphere { center, radius } => *center + unit_or(p - *center, DVec3::Z) * *radius,
            Surface::Torus { center, axis, major, minor } => {
                let (_, _, r) = axial(p - *center, *axis);
                let tube = *center + r * *major;
                tube + unit_or(p - tube, r) * *minor
            }
            Surface::Faceted => p,
        }
    }

    /// Natural unit normal at a point of the surface: the plane normal, otherwise pointing away from the axis, centre or tube centre.
    pub fn normal_at(&self, p: DVec3) -> DVec3 {
        match self {
            Surface::Plane { normal, .. } => *normal,
            Surface::Cylinder { origin, axis, .. } => axial(p - *origin, *axis).2,
            Surface::Cone { apex, axis, half_angle } => {
                let (sin, cos) = half_angle.sin_cos();
                axial(p - *apex, *axis).2 * cos - *axis * sin
            }
            Surface::Sphere { center, .. } => unit_or(p - *center, DVec3::Z),
            Surface::Torus { center, axis, major, .. } => {
                let (_, _, r) = axial(p - *center, *axis);
                unit_or(p - (*center + r * *major), r)
            }
            Surface::Faceted => DVec3::ZERO,
        }
    }

    pub fn distance(&self, p: DVec3) -> f64 {
        (p - self.project(p)).length()
    }

    /// The same surface after rigid motion or mirror `m`: points move as points, directions by the linear part.
    pub fn transformed(&self, m: &DAffine3) -> Surface {
        let (p, d) = (|v: DVec3| m.transform_point3(v), |v: DVec3| m.matrix3 * v);
        match self {
            Surface::Plane { origin, normal } => Surface::Plane { origin: p(*origin), normal: d(*normal) },
            Surface::Cylinder { origin, axis, radius } => Surface::Cylinder { origin: p(*origin), axis: d(*axis), radius: *radius },
            Surface::Cone { apex, axis, half_angle } => Surface::Cone { apex: p(*apex), axis: d(*axis), half_angle: *half_angle },
            Surface::Sphere { center, radius } => Surface::Sphere { center: p(*center), radius: *radius },
            Surface::Torus { center, axis, major, minor } => {
                Surface::Torus { center: p(*center), axis: d(*axis), major: *major, minor: *minor }
            }
            Surface::Faceted => Surface::Faceted,
        }
    }
}
