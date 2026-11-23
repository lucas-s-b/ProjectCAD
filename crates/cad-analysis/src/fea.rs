//! Static linear-elastic finite elements on quadratic tetrahedra (`TetMesh`): element stiffness, consistent loads,
//! sparse assembly over the free degrees of freedom, faer sparse Cholesky, stresses. Units: mm, N, MPa.
use crate::tet_mesh::{TET10_EDGES, TetMesh, tet_volume};
use faer::linalg::solvers::Solve;
use faer::sparse::{SparseColMat, SymbolicSparseColMat};
use faer::{Mat, Side};
use glam::{DMat3, DVec3};
use std::collections::HashMap;

/// Most unknowns (3 per free node) `solve_static` accepts: keeps a study within a few seconds and a few hundred MB.
pub const MAX_DOFS: usize = 30_000;

/// N/mm³ of weight per g/cm³ of density (g = 9.81 m/s²): 1 g/cm³ = 1e-6 kg/mm³, times 9810 mm/s², in 1e-3 N.
const WEIGHT_PER_DENSITY: f64 = 9.81e-6;

const NOT_HELD: &str = "the supports do not hold the body (the stiffness matrix is singular)";

/// Isotropic linear-elastic material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Elastic {
    /// Young's modulus, MPa.
    pub young: f64,
    pub poisson: f64,
}

impl Elastic {
    /// Errors: "Young's modulus must be positive", "Poisson's ratio must be at least 0 and below 0.5".
    pub fn check(&self) -> Result<(), String> {
        if !(self.young > 0.0 && self.young.is_finite()) {
            return Err("Young's modulus must be positive".into());
        }
        if !(0.0..0.5).contains(&self.poisson) {
            return Err("Poisson's ratio must be at least 0 and below 0.5".into());
        }
        Ok(())
    }

    /// Lamé constants (λ, μ) = (E ν / ((1 + ν)(1 − 2ν)), E / (2 (1 + ν))).
    pub fn lame(&self) -> (f64, f64) {
        let (young, nu) = (self.young, self.poisson);
        (young * nu / ((1.0 + nu) * (1.0 - 2.0 * nu)), young / (2.0 * (1.0 + nu)))
    }
}

const GA: f64 = 0.585_410_196_624_968_5; // (5 + 3√5) / 20
const GB: f64 = 0.138_196_601_125_010_5; // (5 − √5) / 20

/// The 4-point Gauss rule on the tetrahedron, in volume coordinates (L0, L1, L2, L3); exact for degree 2, which is
/// the degree of Bᵀ D B for a straight 10-node tetrahedron.
pub const GAUSS: [[f64; 4]; 4] = [[GA, GB, GB, GB], [GB, GA, GB, GB], [GB, GB, GA, GB], [GB, GB, GB, GA]];
/// Weight of each Gauss point on the reference tetrahedron (volume 1/6); multiplied by det J.
pub const GAUSS_WEIGHT: f64 = 1.0 / 24.0;

/// Volume coordinates of the 10 nodes (corners, then the mid-sides of `TET10_EDGES`).
pub const NODE_COORDS: [[f64; 4]; 10] = [
    [1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0],
    [0.5, 0.5, 0.0, 0.0], [0.0, 0.5, 0.5, 0.0], [0.5, 0.0, 0.5, 0.0], [0.5, 0.0, 0.0, 0.5], [0.0, 0.5, 0.0, 0.5], [0.0, 0.0, 0.5, 0.5],
];

/// p qᵀ.
fn outer(p: DVec3, q: DVec3) -> DMat3 {
    DMat3::from_cols(p * q.x, p * q.y, p * q.z)
}

/// Derivatives of the 10 shape functions with respect to the natural coordinates (ξ, η, ζ) = (L1, L2, L3), L0 = 1 −
/// ξ − η − ζ, at volume coordinates `l`. Corner i: N = Lᵢ (2 Lᵢ − 1), dN/dLᵢ = 4 Lᵢ − 1; mid-side node on corners
/// (a, b): N = 4 La Lb, dN/dLa = 4 Lb, dN/dLb = 4 La; then d/dξₖ = d/dLₖ₊₁ − d/dL0.
pub fn shape_derivatives(l: [f64; 4]) -> [DVec3; 10] {
    let mut dl = [[0.0f64; 4]; 10];
    for i in 0..4 {
        dl[i][i] = 4.0 * l[i] - 1.0;
    }
    for (q, [a, b]) in TET10_EDGES.iter().enumerate() {
        dl[4 + q][*a] = 4.0 * l[*b];
        dl[4 + q][*b] = 4.0 * l[*a];
    }
    dl.map(|d| DVec3::new(d[1] - d[0], d[2] - d[0], d[3] - d[0]))
}

/// Gradients of the 10 shape functions in x, y, z at volume coordinates `l` of the element with nodes `x`, and det J
/// (6 times the volume for straight sides). J has the columns ∂x/∂ξ, ∂x/∂η, ∂x/∂ζ = Σ xₙ ⊗ dNₙ; ∇Nₙ = J⁻ᵀ dNₙ.
pub fn gradients(x: &[DVec3; 10], l: [f64; 4]) -> ([DVec3; 10], f64) {
    let d = shape_derivatives(l);
    let j = DMat3::from_cols(
        (0..10).map(|n| x[n] * d[n].x).sum::<DVec3>(),
        (0..10).map(|n| x[n] * d[n].y).sum::<DVec3>(),
        (0..10).map(|n| x[n] * d[n].z).sum::<DVec3>(),
    );
    let jit = j.inverse().transpose();
    (d.map(|dn| jit * dn), j.determinant())
}

/// Element stiffness as 3 x 3 blocks: `k[a][b]` couples the force on node a with the displacement of node b, entry
/// (i, j) = row DOF i of a, column DOF j of b. For isotropic D the product Bᵀ D B per node pair is
/// λ g_a g_bᵀ + μ g_b g_aᵀ + μ (g_a · g_b) 𝟙 (g = shape function gradients), summed over `GAUSS` with w det J.
pub fn element_stiffness(x: &[DVec3; 10], mat: Elastic) -> [[DMat3; 10]; 10] {
    let (lambda, mu) = mat.lame();
    let mut k = [[DMat3::ZERO; 10]; 10];
    for l in GAUSS {
        let (g, det) = gradients(x, l);
        let w = GAUSS_WEIGHT * det;
        for a in 0..10 {
            for b in 0..10 {
                k[a][b] += (outer(g[a], g[b]) * lambda + outer(g[b], g[a]) * mu + DMat3::IDENTITY * (mu * g[a].dot(g[b]))) * w;
            }
        }
    }
    k
}

/// Stress tensor (MPa) from the gradients `g` at a point and the element's nodal displacements `u`:
/// H = Σ uₙ ⊗ gₙ (∂uᵢ/∂xⱼ), ε = (H + Hᵀ) / 2, σ = λ tr(ε) 𝟙 + 2 μ ε.
pub fn stress(g: &[DVec3; 10], u: &[DVec3; 10], mat: Elastic) -> DMat3 {
    let (lambda, mu) = mat.lame();
    let h = (0..10).fold(DMat3::ZERO, |s, n| s + outer(u[n], g[n]));
    let eps = (h + h.transpose()) * 0.5;
    DMat3::IDENTITY * (lambda * (eps.x_axis.x + eps.y_axis.y + eps.z_axis.z)) + eps * (2.0 * mu)
}

/// Von Mises equivalent stress of the symmetric tensor `s`.
pub fn von_mises(s: DMat3) -> f64 {
    let (xx, yy, zz) = (s.x_axis.x, s.y_axis.y, s.z_axis.z);
    let (xy, yz, zx) = (s.y_axis.x, s.z_axis.y, s.x_axis.z);
    (0.5 * ((xx - yy).powi(2) + (yy - zz).powi(2) + (zz - xx).powi(2)) + 3.0 * (xy * xy + yz * yz + zx * zx)).sqrt()
}

/// Node positions of element `k`.
fn element_nodes(mesh: &TetMesh, k: usize) -> [DVec3; 10] {
    mesh.elements[k].map(|v| mesh.nodes[v as usize])
}

/// (smaller corner, larger corner) → mid-side node, from the elements.
fn mid_nodes(mesh: &TetMesh) -> HashMap<(u32, u32), u32> {
    let mut mid = HashMap::new();
    for el in &mesh.elements {
        for (q, [a, b]) in TET10_EDGES.iter().enumerate() {
            mid.insert((el[*a].min(el[*b]), el[*a].max(el[*b])), el[4 + q]);
        }
    }
    mid
}

/// Consistent nodal loads (N, one per node) of a traction (N/mm²) on boundary triangles `tris` of `mesh` (corner
/// triples from `TetMesh::boundary`, ordered outward). `traction` gets the triangle's outward unit normal. A straight
/// 6-node triangle under a uniform traction t gives its corners nothing and each mid-side node t A / 3.
pub fn traction_loads(mesh: &TetMesh, tris: &[[u32; 3]], traction: impl Fn(DVec3) -> DVec3) -> Vec<DVec3> {
    let mid = mid_nodes(mesh);
    let mut f = vec![DVec3::ZERO; mesh.nodes.len()];
    for t in tris {
        let [a, b, c] = t.map(|v| mesh.nodes[v as usize]);
        let cross = (b - a).cross(c - a);
        let share = traction(cross.normalize_or_zero()) * (cross.length() / 6.0);
        for q in 0..3 {
            let (p, r) = (t[q], t[(q + 1) % 3]);
            f[mid[&(p.min(r), p.max(r))] as usize] += share;
        }
    }
    f
}

/// Consistent nodal loads (N) of the body's own weight along −Z at `density` g/cm³: per element of volume V and
/// weight W = density · 9.81e-6 N/mm³ · V, each corner gets −W/20 and each mid-side node W/5 (sum W).
pub fn gravity_loads(mesh: &TetMesh, density: f64) -> Vec<DVec3> {
    let mut f = vec![DVec3::ZERO; mesh.nodes.len()];
    for (k, el) in mesh.elements.iter().enumerate() {
        let weight = DVec3::new(0.0, 0.0, -density * WEIGHT_PER_DENSITY * tet_volume(mesh.corners_of(k)));
        for q in 0..4 {
            f[el[q] as usize] -= weight / 20.0;
        }
        for q in 4..10 {
            f[el[q] as usize] += weight / 5.0;
        }
    }
    f
}

/// Internal nodal forces K u (N) of the displacements `u`: Σ over elements and Gauss points of w det J σ gₙ. Zero at
/// nodes inside a body under a linear displacement field (patch test); at fixed nodes, minus the loads, the reactions.
pub fn internal_forces(mesh: &TetMesh, mat: Elastic, u: &[DVec3]) -> Vec<DVec3> {
    let mut f = vec![DVec3::ZERO; mesh.nodes.len()];
    for (k, el) in mesh.elements.iter().enumerate() {
        let (x, ue) = (element_nodes(mesh, k), el.map(|v| u[v as usize]));
        for l in GAUSS {
            let (g, det) = gradients(&x, l);
            let s = stress(&g, &ue, mat);
            for a in 0..10 {
                f[el[a] as usize] += s * g[a] * (GAUSS_WEIGHT * det);
            }
        }
    }
    f
}

/// Von Mises stress per node (MPa): the stress tensor of every element at each of its 10 nodes (`NODE_COORDS`; exact,
/// since the stress of a straight 10-node tetrahedron is linear), averaged over the elements of the node.
pub fn nodal_von_mises(mesh: &TetMesh, mat: Elastic, u: &[DVec3]) -> Vec<f64> {
    let mut sum = vec![DMat3::ZERO; mesh.nodes.len()];
    let mut count = vec![0u32; mesh.nodes.len()];
    for (k, el) in mesh.elements.iter().enumerate() {
        let (x, ue) = (element_nodes(mesh, k), el.map(|v| u[v as usize]));
        for (q, l) in NODE_COORDS.iter().enumerate() {
            let (g, _) = gradients(&x, *l);
            sum[el[q] as usize] += stress(&g, &ue, mat);
            count[el[q] as usize] += 1;
        }
    }
    sum.iter().zip(&count).map(|(s, &c)| if c == 0 { 0.0 } else { von_mises(*s * (1.0 / f64::from(c))) }).collect()
}

/// A static solution on a `TetMesh`.
#[derive(Debug, Clone, PartialEq)]
pub struct FeaResult {
    /// Per node, mm.
    pub displacement: Vec<DVec3>,
    /// Per node, MPa (`nodal_von_mises`).
    pub von_mises: Vec<f64>,
    /// Sum of the nodal loads, N.
    pub applied: DVec3,
    /// Sum of the support reactions (internal minus external force over the fixed nodes), N; −`applied` in equilibrium.
    pub reaction: DVec3,
    /// |reaction + applied| / Σ |nodal load|: the out-of-balance the solve leaves (round-off, ~1e-12).
    pub residual: f64,
    /// Unknowns solved: 3 per free node.
    pub dofs: usize,
}

/// (largest value, its index); (0, 0) when empty.
fn largest(values: impl Iterator<Item = f64>) -> (f64, usize) {
    values.enumerate().fold((0.0, 0), |best, (i, v)| if v > best.0 { (v, i) } else { best })
}

impl FeaResult {
    /// Largest displacement length, mm, and its node.
    pub fn max_displacement(&self) -> (f64, usize) {
        largest(self.displacement.iter().map(|u| u.length()))
    }

    /// Largest nodal von Mises stress, MPa, and its node.
    pub fn max_von_mises(&self) -> (f64, usize) {
        largest(self.von_mises.iter().copied())
    }
}

/// Solves K u = f with every DOF of the `fixed` nodes held at 0 and nodal loads `loads` (N, one per node of `mesh`).
/// Errors, in this order: `Elastic::check`'s, "fix at least one face" (no fixed node), "no load: add a force, a
/// pressure or gravity" (all loads zero), "the study has 39690 unknowns (at most 30000); use a larger element size",
/// "the supports do not hold the body (the stiffness matrix is singular)" (Cholesky fails, or the out-of-balance
/// exceeds 1e-6 or is not finite).
pub fn solve_static(mesh: &TetMesh, mat: Elastic, fixed: &[u32], loads: &[DVec3]) -> Result<FeaResult, String> {
    mat.check()?;
    if fixed.is_empty() {
        return Err("fix at least one face".into());
    }
    let total: f64 = loads.iter().map(|f| f.length()).sum();
    if total.is_nan() || total <= 0.0 {
        return Err("no load: add a force, a pressure or gravity".into());
    }
    let nodes = mesh.nodes.len();
    let mut held = vec![false; nodes];
    for &v in fixed {
        held[v as usize] = true;
    }
    // Free DOF numbers in DOF order (3 v + i), so they ascend with the node: fixed DOFs are eliminated (u = 0).
    const FIXED: usize = usize::MAX;
    let mut index = vec![FIXED; 3 * nodes];
    let mut n = 0;
    for v in (0..nodes).filter(|&v| !held[v]) {
        for i in 0..3 {
            index[3 * v + i] = n;
            n += 1;
        }
    }
    if n > MAX_DOFS {
        return Err(format!("the study has {n} unknowns (at most {MAX_DOFS}); use a larger element size"));
    }
    // Lower-triangle CSC pattern: column (v, i) holds the free rows ≥ it of every node sharing an element with v.
    let mut near: Vec<Vec<u32>> = vec![Vec::new(); nodes];
    for el in &mesh.elements {
        for &a in el {
            near[a as usize].extend_from_slice(el);
        }
    }
    let (mut col_ptr, mut row_idx) = (vec![0usize; n + 1], Vec::new());
    for v in 0..nodes {
        near[v].sort_unstable();
        near[v].dedup();
        for i in 0..3 {
            let col = index[3 * v + i];
            if col == FIXED {
                continue;
            }
            for &m in &near[v] {
                for j in 0..3 {
                    let row = index[3 * m as usize + j];
                    if row != FIXED && row >= col {
                        row_idx.push(row);
                    }
                }
            }
            col_ptr[col + 1] = row_idx.len();
        }
    }
    drop(near);
    let mut val = vec![0.0; row_idx.len()];
    for (k, el) in mesh.elements.iter().enumerate() {
        let ke = element_stiffness(&element_nodes(mesh, k), mat);
        for b in 0..10 {
            for j in 0..3 {
                let col = index[3 * el[b] as usize + j];
                if col == FIXED {
                    continue;
                }
                let rows = &row_idx[col_ptr[col]..col_ptr[col + 1]];
                for a in 0..10 {
                    for i in 0..3 {
                        let row = index[3 * el[a] as usize + i];
                        if row == FIXED || row < col {
                            continue;
                        }
                        let at = rows.binary_search(&row).expect("the pattern holds every element pair");
                        val[col_ptr[col] + at] += ke[a][b].col(j)[i];
                    }
                }
            }
        }
    }
    let mut rhs = vec![0.0; n];
    for v in 0..nodes {
        for i in 0..3 {
            if index[3 * v + i] != FIXED {
                rhs[index[3 * v + i]] = loads[v][i];
            }
        }
    }
    let symbolic = SymbolicSparseColMat::<usize>::new_checked(n, n, col_ptr, None, row_idx);
    let k = SparseColMat::<usize, f64>::new(symbolic, val);
    let llt = k.sp_cholesky(Side::Lower).map_err(|_| NOT_HELD.to_string())?;
    let x = llt.solve(&Mat::from_fn(n, 1, |d, _| rhs[d]));
    let displacement: Vec<DVec3> = (0..nodes)
        .map(|v| DVec3::from_array([0, 1, 2].map(|i| if index[3 * v + i] == FIXED { 0.0 } else { x[(index[3 * v + i], 0)] })))
        .collect();
    let inner = internal_forces(mesh, mat, &displacement);
    let applied: DVec3 = loads.iter().sum();
    let reaction: DVec3 = (0..nodes).filter(|&v| held[v]).map(|v| inner[v] - loads[v]).sum();
    let residual = (reaction + applied).length() / total;
    if residual.is_nan() || residual > 1e-6 {
        return Err(NOT_HELD.into());
    }
    Ok(FeaResult { von_mises: nodal_von_mises(mesh, mat, &displacement), displacement, applied, reaction, residual, dofs: n })
}

#[cfg(test)]
#[path = "fea_tests.rs"]
mod tests;
