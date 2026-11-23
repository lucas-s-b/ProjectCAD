use super::*;
use crate::mass::tests::box_mesh;
use crate::tet_mesh;
use glam::{DMat3, DVec3};

/// Corners plus straight mid-side nodes in `TET10_EDGES` order.
fn tet10(c: [DVec3; 4]) -> [DVec3; 10] {
    let mut x = [DVec3::ZERO; 10];
    x[..4].copy_from_slice(&c);
    for (q, [a, b]) in TET10_EDGES.iter().enumerate() {
        x[4 + q] = (c[*a] + c[*b]) / 2.0;
    }
    x
}
/// The Kuhn tetrahedron of the unit cube.
fn kuhn() -> [DVec3; 10] {
    tet10([DVec3::ZERO, DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::ONE])
}
const STEEL: Elastic = Elastic { young: 210_000.0, poisson: 0.3 };
/// A linear displacement field (∂u/∂x has no symmetry) plus a translation.
fn linear(p: DVec3) -> DVec3 {
    DVec3::new(1e-3 * p.x + 2e-4 * p.y, -1e-4 * p.x - 5e-4 * p.y + 3e-4 * p.z, 2e-4 * p.x + 4e-4 * p.z) + DVec3::new(0.1, -0.2, 0.3)
}
/// Nodes of `m` with coordinate `axis` equal to `value` (exact: box grids land on the faces).
fn on(m: &TetMesh, axis: usize, value: f64) -> Vec<u32> {
    (0..m.nodes.len() as u32).filter(|&v| m.nodes[v as usize][axis] == value).collect()
}
/// Boundary triangles of `m` with every corner at coordinate `axis` = `value`.
fn tris_on(m: &TetMesh, axis: usize, value: f64) -> Vec<[u32; 3]> {
    m.boundary().into_iter().filter(|t| t.iter().all(|&v| m.nodes[v as usize][axis] == value)).collect()
}
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} vs {b} (tolerance {t})");
}
fn close3(a: DVec3, b: DVec3, t: f64) {
    assert!(a.distance(b) <= t, "{a} vs {b} (tolerance {t})");
}
fn box20() -> TetMesh {
    tet_mesh(&box_mesh(DVec3::ZERO, DVec3::new(20.0, 10.0, 10.0)), 5.0).unwrap()
}

#[test]
fn gauss_rule() {
    close(4.0 * GAUSS_WEIGHT, 1.0 / 6.0, 1e-15);
    for row in GAUSS {
        close(row.iter().sum::<f64>(), 1.0, 1e-15);
    }
    close(GAUSS.iter().map(|l| GAUSS_WEIGHT * l[0] * l[0]).sum::<f64>(), 1.0 / 60.0, 1e-15);
    close(GAUSS.iter().map(|l| GAUSS_WEIGHT * l[0] * l[1]).sum::<f64>(), 1.0 / 120.0, 1e-15);
}

#[test]
fn gradients_of_linear_fields() {
    let x = kuhn();
    let a = DVec3::new(0.3, -1.2, 2.5);
    let mut points = GAUSS.to_vec();
    points.push([0.3, 0.2, 0.1, 0.4]);
    for l in points {
        let d = shape_derivatives(l);
        assert!(d.iter().copied().sum::<DVec3>().length() <= 1e-14);
        let (g, det) = gradients(&x, l);
        close(det, 1.0, 1e-12);
        let grad: DVec3 = (0..10).map(|n| g[n] * (a.dot(x[n]) + 7.0)).sum();
        close3(grad, a, 1e-12);
    }
}

#[test]
fn element_stiffness_is_b_d_b_with_six_rigid_modes() {
    let k = element_stiffness(&kuhn(), STEEL);
    let kk = faer::Mat::<f64>::from_fn(30, 30, |r, c| k[r / 3][c / 3].col(c % 3)[r % 3]);
    let m = (0..30).flat_map(|r| (0..30).map(move |c| (r, c))).map(|(r, c)| kk[(r, c)].abs()).fold(0.0, f64::max);
    for r in 0..30 {
        for c in 0..30 {
            assert!((kk[(r, c)] - kk[(c, r)]).abs() <= 1e-9 * m);
        }
    }
    let (lambda, mu) = STEEL.lame();
    let mut d = [[0.0; 6]; 6]; // Voigt order xx, yy, zz, xy, yz, zx (engineering shear)
    for i in 0..3 {
        for (j, entry) in d[i].iter_mut().take(3).enumerate() {
            *entry = lambda + if i == j { 2.0 * mu } else { 0.0 };
        }
        d[3 + i][3 + i] = mu;
    }
    let mut kbd = [[0.0; 30]; 30];
    for l in GAUSS {
        let (g, det) = gradients(&kuhn(), l);
        let mut b = [[0.0; 30]; 6];
        for (n, gn) in g.iter().enumerate() {
            let c = 3 * n;
            (b[0][c], b[1][c + 1], b[2][c + 2]) = (gn.x, gn.y, gn.z);
            (b[3][c], b[3][c + 1]) = (gn.y, gn.x);
            (b[4][c + 1], b[4][c + 2]) = (gn.z, gn.y);
            (b[5][c], b[5][c + 2]) = (gn.z, gn.x);
        }
        for r in 0..30 {
            for c in 0..30 {
                let s: f64 = (0..6).map(|p| (0..6).map(|q| b[p][r] * d[p][q] * b[q][c]).sum::<f64>()).sum();
                kbd[r][c] += s * GAUSS_WEIGHT * det;
            }
        }
    }
    for r in 0..30 {
        for c in 0..30 {
            assert!((kk[(r, c)] - kbd[r][c]).abs() <= 1e-9 * m, "entry {r},{c}");
        }
    }
    let x = kuhn();
    let modes: Vec<[DVec3; 10]> = [DVec3::X, DVec3::Y, DVec3::Z]
        .into_iter()
        .flat_map(|e_i| [x.map(|_| e_i), x.map(|p| e_i.cross(p))])
        .collect();
    for u in modes {
        for r in 0..30 {
            let f: f64 = (0..30).map(|c| kk[(r, c)] * u[c / 3][c % 3]).sum();
            assert!(f.abs() <= 1e-9 * m, "row {r}: {f}");
        }
    }
    let mut ev: Vec<f64> = kk.self_adjoint_eigenvalues(Side::Lower).unwrap();
    ev.sort_by(f64::total_cmp);
    let max = ev[29];
    assert!(ev[..6].iter().all(|v| v.abs() <= 1e-9 * max), "{:?}", &ev[..7]);
    assert!(ev[6] >= 1e-3 * max, "{}", ev[6] / max);
}

#[test]
fn stress_of_a_linear_field() {
    let x = kuhn();
    let u = x.map(linear);
    close(STEEL.lame().0, 121153.84615384616, 1e-8);
    close(STEEL.lame().1, 80769.23076923077, 1e-8);
    let mut points = GAUSS.to_vec();
    points.extend(NODE_COORDS);
    for l in points {
        let (g, _) = gradients(&x, l);
        let s = stress(&g, &u, STEEL);
        let got = [s.x_axis.x, s.y_axis.y, s.z_axis.z, s.y_axis.x, s.z_axis.y, s.x_axis.z];
        let want = [270.576_923_076_923_1, 28.269230769230774, 173.65384615384613, 8.076923076923077, 24.23076923076923, 16.153846153846153];
        for (g, w) in got.iter().zip(want) {
            close(*g, w, 1e-8);
        }
        close(s.y_axis.x, s.x_axis.y, 1e-9);
        close(s.z_axis.y, s.y_axis.z, 1e-9);
        close(s.x_axis.z, s.z_axis.x, 1e-9);
        close(von_mises(s), 217.62774253188942, 1e-8);
    }
}

#[test]
fn von_mises_cases() {
    close(von_mises(DMat3::from_diagonal(DVec3::new(10.0, 0.0, 0.0))), 10.0, 1e-12);
    let mut shear = DMat3::ZERO;
    shear.y_axis.x = 5.0;
    shear.x_axis.y = 5.0;
    close(von_mises(shear), 8.660254037844387, 1e-12);
    close(von_mises(DMat3::from_diagonal(DVec3::splat(-7.0))), 0.0, 1e-12);
}

#[test]
fn linear_field_leaves_no_force_inside() {
    let m = box20();
    let u: Vec<DVec3> = m.nodes.iter().map(|p| linear(*p)).collect();
    let f = internal_forces(&m, STEEL, &u);
    let fmax = f.iter().map(|v| v.length()).fold(0.0, f64::max);
    let inside: Vec<usize> = (0..m.nodes.len())
        .filter(|&v| {
            let p = m.nodes[v];
            p.x > 0.0 && p.x < 20.0 && p.y > 0.0 && p.y < 10.0 && p.z > 0.0 && p.z < 10.0
        })
        .collect();
    assert_eq!(inside.len(), 63);
    for v in inside {
        assert!(f[v].length() <= 1e-9 * fmax, "node {v}");
    }
    assert!(f.iter().copied().sum::<DVec3>().length() <= 1e-9 * fmax);
}

#[test]
fn loads_sum_to_their_totals() {
    let m = box20();
    let tris = tris_on(&m, 0, 20.0);
    assert_eq!(tris.len(), 8);
    let f = traction_loads(&m, &tris, |_| DVec3::new(10.0, 0.0, 0.0));
    close3(f.iter().copied().sum(), DVec3::new(1000.0, 0.0, 0.0), 1e-9);
    assert!(f[..m.corners].iter().all(|v| *v == DVec3::ZERO));
    let f = traction_loads(&m, &tris, |n| -n * 10.0);
    close3(f.iter().copied().sum(), DVec3::new(-1000.0, 0.0, 0.0), 1e-9);
    let g = gravity_loads(&m, 7.85);
    close3(g.iter().copied().sum(), DVec3::new(0.0, 0.0, -7.85 * 9.81e-6 * 2000.0), 1e-12);
}

#[test]
fn tension_bar_is_exact() {
    let m = tet_mesh(&box_mesh(DVec3::ZERO, DVec3::new(100.0, 10.0, 10.0)), 5.0).unwrap();
    assert_eq!(m.nodes.len(), 1025);
    let fixed = on(&m, 0, 0.0);
    assert_eq!(fixed.len(), 25);
    let loads = traction_loads(&m, &tris_on(&m, 0, 100.0), |_| DVec3::new(10.0, 0.0, 0.0));
    let r = solve_static(&m, Elastic { young: 210_000.0, poisson: 0.0 }, &fixed, &loads).unwrap();
    assert_eq!(r.dofs, 3000);
    for (p, u) in m.nodes.iter().zip(&r.displacement) {
        close(u.x, 10.0 * p.x / 210_000.0, 1e-11);
        assert!(u.y.abs() <= 1e-11 && u.z.abs() <= 1e-11);
    }
    assert!(r.von_mises.iter().all(|s| (s - 10.0).abs() <= 1e-8));
    let (d, node) = r.max_displacement();
    close(d, 0.004761904761904762, 1e-11);
    assert_eq!(m.nodes[node].x, 100.0);
    close3(r.applied, DVec3::new(1000.0, 0.0, 0.0), 1e-9);
    close3(r.reaction, DVec3::new(-1000.0, 0.0, 0.0), 1e-6);
    assert!(r.residual <= 1e-9);
}

#[test]
fn solver_refusals() {
    let m = box20();
    let fixed = on(&m, 0, 0.0);
    let mut loads = vec![DVec3::ZERO; m.nodes.len()];
    *loads.last_mut().unwrap() = DVec3::new(0.0, 0.0, -1.0);
    let run = |young, poisson, fixed: &[u32], loads: &[DVec3]| solve_static(&m, Elastic { young, poisson }, fixed, loads).map(drop);
    assert_eq!(run(0.0, 0.3, &fixed, &loads), Err("Young's modulus must be positive".to_string()));
    for nu in [0.5, -0.1] {
        assert_eq!(run(210_000.0, nu, &fixed, &loads), Err("Poisson's ratio must be at least 0 and below 0.5".to_string()));
    }
    assert_eq!(run(210_000.0, 0.3, &[], &loads), Err("fix at least one face".to_string()));
    assert_eq!(run(210_000.0, 0.3, &fixed, &vec![DVec3::ZERO; m.nodes.len()]), Err("no load: add a force, a pressure or gravity".to_string()));
    let big = tet_mesh(&box_mesh(DVec3::ZERO, DVec3::new(30.0, 20.0, 20.0)), 2.0).unwrap();
    assert_eq!(big.nodes.len(), 13_671);
    let fixed = on(&big, 0, 0.0);
    assert_eq!(fixed.len(), 441);
    let mut loads = vec![DVec3::ZERO; big.nodes.len()];
    *loads.last_mut().unwrap() = DVec3::new(0.0, 0.0, -1.0);
    assert_eq!(solve_static(&big, STEEL, &fixed, &loads).map(drop), Err("the study has 39690 unknowns (at most 30000); use a larger element size".to_string()));
}
