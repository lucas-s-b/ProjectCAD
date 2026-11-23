//! Mass properties of closed triangle meshes by the divergence theorem (signed tetrahedra to a reference point).
use cad_modeler::TriMesh;
use glam::{DMat3, DVec3};

/// Mass properties of a solid in the coordinates of its mesh. Units: mm, mm³, g, g·mm².
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassProps {
    /// mm³
    pub volume: f64,
    /// g
    pub mass: f64,
    /// Centre of mass, mm.
    pub com: DVec3,
    /// Inertia tensor about the centre of mass with axes parallel to the mesh axes, g·mm²: the diagonal holds
    /// Ixx = ∫ρ(y² + z²) dV and so on, the off-diagonal entries the products with the minus sign (-∫ρxy dV).
    pub inertia: DMat3,
}

/// `v` vᵀ.
fn outer(v: DVec3) -> DMat3 {
    DMat3::from_cols(v * v.x, v * v.y, v * v.z)
}

/// The inertia of a point mass `m` at offset `d`: m (|d|² 𝟙 - d dᵀ).
fn point_inertia(m: f64, d: DVec3) -> DMat3 {
    (DMat3::IDENTITY * d.length_squared() - outer(d)) * m
}

impl MassProps {
    /// Mass properties of the closed, outward-oriented `mesh` at `density` g/cm³. Exact for the mesh: for each
    /// triangle (a, b, c) relative to the reference point r (the first vertex, which keeps far-off meshes precise),
    /// d = a · (b × c); V = Σ d / 6; first moment Σ d (a + b + c) / 24; second moment C = Σ d / 120 (a aᵀ + b bᵀ + c cᵀ + s sᵀ)
    /// with s = a + b + c. With c = first moment / V and C_c = C - V c cᵀ the inertia about the centre of mass is
    /// ρ (tr(C_c) 𝟙 - C_c), ρ = density / 1000 g/mm³. None when the volume is not positive and finite (open or inside-out mesh).
    pub fn of(mesh: &TriMesh, density: f64) -> Option<MassProps> {
        let r = *mesh.positions.first()?;
        let (mut six_v, mut first, mut second) = (0.0, DVec3::ZERO, DMat3::ZERO);
        for t in mesh.indices.as_chunks::<3>().0 {
            let [a, b, c] = t.map(|i| mesh.positions[i as usize] - r);
            let d = a.dot(b.cross(c));
            let s = a + b + c;
            six_v += d;
            first += s * d;
            second += (outer(a) + outer(b) + outer(c) + outer(s)) * d;
        }
        let volume = six_v / 6.0;
        if !(volume > 0.0 && volume.is_finite()) {
            return None;
        }
        let com = first / 24.0 / volume;
        let cov = second / 120.0 - outer(com) * volume;
        let rho = density / 1000.0;
        let trace = cov.x_axis.x + cov.y_axis.y + cov.z_axis.z;
        Some(MassProps { volume, mass: volume * rho, com: r + com, inertia: (DMat3::IDENTITY * trace - cov) * rho })
    }

    /// The inertia tensor about the point `p` with the same axes (parallel axis theorem): I + m (|d|² 𝟙 - d dᵀ), d = com - p.
    pub fn inertia_at(&self, p: DVec3) -> DMat3 {
        self.inertia + point_inertia(self.mass, self.com - p)
    }

    /// Several solids as one: masses and volumes add, the centre of mass is mass-weighted and the inertia is taken
    /// about it with the parallel axis theorem. None for an empty list or a total mass that is not positive.
    pub fn combine(parts: &[MassProps]) -> Option<MassProps> {
        let mass: f64 = parts.iter().map(|p| p.mass).sum();
        if mass.is_nan() || mass <= 0.0 {
            return None;
        }
        let com = parts.iter().map(|p| p.com * p.mass).sum::<DVec3>() / mass;
        let inertia = parts.iter().fold(DMat3::ZERO, |acc, p| acc + p.inertia_at(com));
        Some(MassProps { volume: parts.iter().map(|p| p.volume).sum(), mass, com, inertia })
    }

    /// Principal moments of inertia about the centre of mass, ascending (g·mm²), and their unit axes. Each of the
    /// first two axes is turned so that its component of largest magnitude is positive; the third is their cross
    /// product (a right-handed frame). Equal moments give any orthonormal axes of that subspace.
    pub fn principal(&self) -> ([f64; 3], [DVec3; 3]) {
        let (values, vectors) = eigen_sym(self.inertia);
        let mut order = [0, 1, 2];
        order.sort_by(|&i, &j| values[i].total_cmp(&values[j]));
        let positive = |v: DVec3| {
            let big = if v.x.abs() >= v.y.abs() && v.x.abs() >= v.z.abs() { v.x } else if v.y.abs() >= v.z.abs() { v.y } else { v.z };
            if big < 0.0 { -v } else { v }
        };
        let (a, b) = (positive(vectors[order[0]]), positive(vectors[order[1]]));
        (order.map(|i| values[i]), [a, b, a.cross(b)])
    }
}

/// Eigenvalues and unit eigenvectors (vector k belongs to value k) of the symmetric matrix `m`, by cyclic Jacobi
/// rotations; converges in a few sweeps for 3 x 3.
fn eigen_sym(m: DMat3) -> ([f64; 3], [DVec3; 3]) {
    // Row-major copies: a[i][j]; v holds the eigenvectors as columns, v[row][k].
    let mut a = m.transpose().to_cols_array_2d();
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let scale = (0..3).map(|i| a[i][i].abs()).fold(f64::MIN_POSITIVE, f64::max);
    for _ in 0..50 {
        if a[0][1].abs() + a[0][2].abs() + a[1][2].abs() <= 1e-15 * scale {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q] == 0.0 {
                continue;
            }
            // Rotation in the (p, q) plane that zeroes a[p][q]: A' = Jᵀ A J, V' = V J.
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            for row in &mut a {
                let (kp, kq) = (row[p], row[q]);
                row[p] = c * kp - s * kq;
                row[q] = s * kp + c * kq;
            }
            let (rp, rq) = (a[p], a[q]);
            a[p] = std::array::from_fn(|k| c * rp[k] - s * rq[k]);
            a[q] = std::array::from_fn(|k| s * rp[k] + c * rq[k]);
            for row in &mut v {
                let (kp, kq) = (row[p], row[q]);
                row[p] = c * kp - s * kq;
                row[q] = s * kp + c * kq;
            }
        }
    }
    let axis = |k: usize| DVec3::new(v[0][k], v[1][k], v[2][k]);
    ([a[0][0], a[1][1], a[2][2]], [axis(0), axis(1), axis(2)])
}

#[cfg(test)]
#[path = "mass_tests.rs"]
mod tests;
