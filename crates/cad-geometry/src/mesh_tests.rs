use super::*;

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn v3(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn solids() -> Vec<Shape> {
    vec![
        Shape::Box { center: v3(1.0, 2.0, 3.0), size: v3(2.0, 4.0, 6.0) },
        Shape::Cylinder { center: v3(0.0, 1.0, 2.0), radius: 3.0, height: 5.0 },
        Shape::Sphere { center: v3(-1.0, 0.0, 4.0), radius: 2.5 },
    ]
}

#[test]
fn triangle_counts() {
    let tris = |s: &Shape| tessellate(s).map(|m| m.positions.len() / 3);
    let s = solids();
    assert_eq!(tris(&s[0]), Some(12));
    assert_eq!(tris(&s[1]), Some(CYL_SEGMENTS * 4));
    assert_eq!(tris(&s[2]), Some(SPHERE_SEGMENTS * (2 * SPHERE_RINGS - 2)));
    assert!(tessellate(&Shape::Circle { center: v2(0.0, 0.0), radius: 1.0 }).is_none());
}

#[test]
fn normals_unit_and_positions_in_bbox() {
    for s in solids() {
        let m = tessellate(&s).unwrap();
        assert_eq!(m.positions.len(), m.normals.len());
        let (lo, hi) = s.bbox();
        for (p, n) in m.positions.iter().zip(&m.normals) {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "{s:?}");
            for k in 0..3 {
                assert!(p[k] as f64 >= lo[k] - 1e-4 && p[k] as f64 <= hi[k] + 1e-4, "{s:?}");
            }
        }
    }
}

#[test]
fn outline_counts() {
    let n = |s: &Shape| outline(s).len();
    assert_eq!(n(&Shape::Line { a: v2(0.0, 0.0), b: v2(1.0, 1.0) }), 1);
    assert_eq!(n(&Shape::rect(v2(0.0, 0.0), v2(1.0, 1.0))), 4);
    assert_eq!(n(&Shape::Circle { center: v2(0.0, 0.0), radius: 1.0 }), 64);
    let s = solids();
    assert_eq!(n(&s[0]), 12);
    assert_eq!(n(&s[1]), 64 * 2 + 4);
    assert_eq!(n(&s[2]), 64 * 3);
    let arc = |sweep: f64| Shape::Arc { center: v2(0.0, 0.0), radius: 1.0, start: 0.0, sweep };
    assert_eq!(n(&arc(TAU)), 64);
    assert_eq!(n(&arc(TAU / 4.0)), 16);
    assert_eq!(n(&arc(0.01)), 2);
    let pl = |closed| Shape::Polyline { points: vec![v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, 1.0)], closed };
    assert_eq!(n(&pl(false)), 2);
    assert_eq!(n(&pl(true)), 3);
    let poly = pl(false);
    assert!(tessellate(&poly).is_none() && tessellate(&arc(1.0)).is_none());
}

#[test]
fn box_edge_lengths() {
    let b = Shape::Box { center: v3(0.0, 0.0, 0.0), size: v3(2.0, 4.0, 6.0) };
    let mut lens: Vec<f64> = outline(&b).iter().map(|[p, q]| p.distance(*q)).collect();
    lens.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(lens, [2.0, 2.0, 2.0, 2.0, 4.0, 4.0, 4.0, 4.0, 6.0, 6.0, 6.0, 6.0]);
}

#[test]
fn paths_flatten_to_outline() {
    let mut shapes = solids();
    shapes.extend([
        Shape::Line { a: v2(0.0, 0.0), b: v2(1.0, 1.0) },
        Shape::rect(v2(0.0, 0.0), v2(2.0, 1.0)),
        Shape::Circle { center: v2(1.0, 1.0), radius: 2.0 },
        Shape::Arc { center: v2(0.0, 0.0), radius: 1.0, start: 0.5, sweep: 2.0 },
        Shape::Polyline { points: vec![v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, 1.0)], closed: true },
        Shape::Polyline { points: vec![v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, 1.0)], closed: false },
        Shape::Text { pos: v2(0.0, 0.0), height: 2.0, angle: 0.3, text: "ab".into() },
        Shape::Dimension { a: v2(0.0, 0.0), b: v2(10.0, 0.0), offset: 3.0, text_height: 1.0 },
    ]);
    for s in &shapes {
        let flat: Vec<[DVec3; 2]> = outline_paths(s).iter().flat_map(|p| p.windows(2).map(|w| [w[0], w[1]])).collect();
        assert_eq!(flat, outline(s), "{s:?}");
    }
    let circle = &outline_paths(&Shape::Circle { center: v2(0.0, 0.0), radius: 1.0 })[0];
    assert_eq!((circle.len(), circle[0]), (65, circle[64]));
    assert_eq!(outline_paths(&Shape::Dimension { a: v2(0.0, 0.0), b: v2(10.0, 0.0), offset: 3.0, text_height: 1.0 }).len(), 5);
}
