use crate::brep::{Brep, CoEdge, Edge, Face, TopoError};
use crate::geom::{Curve, Surface};
use glam::DVec3;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// Unit cube. Faces: 0 bottom, 1 top, 2 front (y=0), 3 right (x=1), 4 back (y=1), 5 left (x=0).
fn cube() -> Brep {
    let vertices = vec![
        v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0), v(0.0, 1.0, 0.0),
        v(0.0, 0.0, 1.0), v(1.0, 0.0, 1.0), v(1.0, 1.0, 1.0), v(0.0, 1.0, 1.0),
    ];
    let pairs = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    let edges = pairs.iter().map(|&(a, b)| Edge { v: [a, b], curve: Curve::Line }).collect();
    let face = |origin: DVec3, normal: DVec3, outward: bool, l: [(u32, bool); 4]| Face {
        surface: Surface::Plane { origin, normal },
        outward,
        loops: vec![l.iter().map(|&(edge, reversed)| CoEdge { edge, reversed }).collect()],
    };
    let faces = vec![
        face(v(0.0, 0.0, 0.0), DVec3::Z, false, [(3, true), (2, true), (1, true), (0, true)]),
        face(v(0.0, 0.0, 1.0), DVec3::Z, true, [(4, false), (5, false), (6, false), (7, false)]),
        face(v(0.0, 0.0, 0.0), -DVec3::Y, true, [(0, false), (9, false), (4, true), (8, true)]),
        face(v(1.0, 0.0, 0.0), DVec3::X, true, [(1, false), (10, false), (5, true), (9, true)]),
        face(v(0.0, 1.0, 0.0), DVec3::Y, true, [(2, false), (11, false), (6, true), (10, true)]),
        face(v(0.0, 0.0, 0.0), -DVec3::X, true, [(3, false), (8, false), (7, true), (11, true)]),
    ];
    Brep { vertices, edges, faces }
}

#[test]
fn cube_is_valid() {
    let c = cube();
    assert_eq!(c.validate(), Ok(()));
    assert_eq!(c.euler(), 2);
    assert_eq!(c.faces_of_edge(0), vec![0, 2]);
    assert_eq!(c.edges_of_vertex(0), vec![0, 3, 8]);
    assert_eq!(c.adjacent_faces(1), vec![2, 3, 4, 5]);
    assert_eq!(c.coedge_ends(CoEdge { edge: 3, reversed: true }), (0, 3));
    assert_eq!(c.bbox(), (DVec3::ZERO, DVec3::ONE));
}

#[test]
fn flipped_coedge_is_not_manifold() {
    let mut c = cube();
    c.faces[1].loops[0][0].reversed = true;
    assert_eq!(c.validate(), Err(TopoError::NotManifold(4, 2)));
}

#[test]
fn missing_face_is_not_manifold() {
    let mut c = cube();
    c.faces.pop();
    assert!(matches!(c.validate(), Err(TopoError::NotManifold(_, 1))));
}

#[test]
fn vertex_off_plane() {
    let mut c = cube();
    c.vertices[7].z = 1.5;
    match c.validate() {
        Err(TopoError::OffSurface(1, d)) => assert!((d - 0.5).abs() < 1e-12),
        other => panic!("{other:?}"),
    }
}

#[test]
fn open_loop_and_bad_index() {
    let mut c = cube();
    c.faces[0].loops[0].swap(0, 1);
    assert_eq!(c.validate(), Err(TopoError::OpenLoop(0)));
    let mut c = cube();
    c.edges[0].v[1] = 99;
    assert_eq!(c.validate(), Err(TopoError::BadIndex));
    let mut c = cube();
    c.vertices[3].x = f64::NAN;
    assert_eq!(c.validate(), Err(TopoError::NonFinite));
    let mut c = cube();
    c.vertices[1] = c.vertices[0];
    assert_eq!(c.validate(), Err(TopoError::DegenerateEdge(0)));
    assert_eq!(Brep::default().validate(), Err(TopoError::Empty));
}

#[test]
fn faceted_face_is_rejected() {
    let mut c = cube();
    c.faces[3].surface = Surface::Faceted;
    assert_eq!(c.validate(), Err(TopoError::NoSurface(3)));
}

#[test]
fn arc_data_is_checked() {
    let arc = |center: DVec3, sweep: f64| Curve::Arc { center, normal: DVec3::Z, sweep };
    let mut c = cube();
    // Half circle in the bottom plane from vertex 0 to vertex 1; its midpoint leaves the front plane.
    c.edges[0].curve = arc(v(0.5, 0.0, 0.0), std::f64::consts::PI);
    assert!(matches!(c.validate(), Err(TopoError::OffSurface(..))));
    c.edges[0].curve = arc(v(0.5, 1.0, 0.0), std::f64::consts::PI);
    assert_eq!(c.validate(), Err(TopoError::BadArc(0)));
    c.edges[0].curve = arc(v(0.5, 0.0, 0.0), 1.0);
    assert_eq!(c.validate(), Err(TopoError::BadArc(0)));
    c.edges[0].curve = Curve::Arc { center: v(0.5, 0.0, 0.0), normal: DVec3::Z * 2.0, sweep: std::f64::consts::PI };
    assert_eq!(c.validate(), Err(TopoError::BadArc(0)));
}

#[test]
fn bbox_includes_arc_extremes() {
    let mut c = cube();
    c.edges[0].curve = Curve::Arc { center: v(0.5, 0.0, 0.0), normal: DVec3::Z, sweep: std::f64::consts::PI };
    let (lo, hi) = c.bbox();
    assert!(lo.distance(v(0.0, -0.5, 0.0)) < 1e-12, "{lo:?}");
    assert!(hi.distance(v(1.0, 1.0, 1.0)) < 1e-12);
}

#[test]
fn serde_round_trip() {
    let c = cube();
    let s = serde_json::to_string(&c).unwrap();
    assert_eq!(serde_json::from_str::<Brep>(&s).unwrap(), c);
}
