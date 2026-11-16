use super::*;
use crate::fixtures::*;
use crate::{BoolOp, EdgeGeom, boolean, extrude};
use cad_geometry::profile::chain;
use cad_geometry::shape::Shape;
use std::f64::consts::{FRAC_PI_2, PI};

const TOL: f64 = 0.01;

fn yz() -> Plane3 {
    Plane3 { origin: DVec3::ZERO, x: DVec3::Y, y: DVec3::Z }
}

fn line(a: (f64, f64), b: (f64, f64)) -> Shape {
    Shape::Line { a: v2(a.0, a.1), b: v2(b.0, b.1) }
}

fn pieces(shapes: &[Shape]) -> (Vec<PathPiece>, bool) {
    let refs: Vec<&Shape> = shapes.iter().collect();
    let (segs, closed) = chain(&refs).unwrap();
    (path_pieces(&segs, &Plane3::XY, TOL), closed)
}

fn run(profile: &Profile, plane: &Plane3, shapes: &[Shape]) -> Result<Body, ModelError> {
    let (p, closed) = pieces(shapes);
    sweep(profile, plane, &p, closed, TOL)
}

/// Area of the chord polygon a circle of radius `r` is sampled as.
fn facet_area(r: f64) -> f64 {
    let n = 2.0 * arc_steps(r, PI, TOL) as f64;
    0.5 * n * r * r * (2.0 * PI / n).sin()
}

/// Length of the chord polygon an arc of radius `r` and sweep `sweep` is sampled as.
fn facet_length(r: f64, sweep: f64) -> f64 {
    let n = arc_steps(r, sweep, TOL) as f64;
    n * 2.0 * r * (sweep / n / 2.0).sin()
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs().max(1.0), "{a} vs {b}");
}

fn count(b: &Body, f: impl Fn(&Surface) -> bool) -> usize {
    b.faces.iter().filter(|x| f(&x.surface)).count()
}

fn rect_profile() -> Profile {
    make(&[rect(-2.0, -1.0, 2.0, 1.0)])
}

fn l_path() -> [Shape; 2] {
    [line((0.0, 0.0), (30.0, 0.0)), line((30.0, 0.0), (30.0, 20.0))]
}

#[test]
fn straight_rectangle_equals_the_extrusion() {
    let b = run(&rect_profile(), &yz(), &[line((0.0, 0.0), (30.0, 0.0))]).unwrap();
    assert!(b.mesh.is_closed());
    assert!((b.volume() - 240.0).abs() < 1e-9);
    assert_eq!(b.faces.len(), 6);
    assert_eq!(count(&b, |s| matches!(s, Surface::Plane { .. })), 6);
    let e = Body::from_brep(&extrude(&rect_profile(), &yz(), 30.0).unwrap(), TOL).unwrap();
    assert!((b.volume() - e.volume()).abs() < 1e-9 && b.faces.len() == e.faces.len());
    assert!((b.area() - e.area()).abs() < 1e-9);
}

#[test]
fn reversed_profile_plane_is_still_outward() {
    let flipped = Plane3 { origin: DVec3::ZERO, x: DVec3::Z, y: DVec3::Y };
    let b = run(&rect_profile(), &flipped, &[line((0.0, 0.0), (30.0, 0.0))]).unwrap();
    assert!(b.mesh.is_closed());
    assert!((b.volume() - 240.0).abs() < 1e-9);
}

#[test]
fn straight_circle_has_one_cylinder_side() {
    let b = run(&make(&[circle(0.0, 0.0, 2.0)]), &yz(), &[line((0.0, 0.0), (30.0, 0.0))]).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), facet_area(2.0) * 30.0, 1e-9);
    assert_eq!(b.faces.len(), 3);
    assert_eq!(count(&b, |s| matches!(s, Surface::Cylinder { radius, .. } if (*radius - 2.0).abs() < 1e-12)), 1);
    for (k, &i) in b.mesh.indices.iter().enumerate() {
        let (f, p) = (&b.faces[b.tri_face[k / 3] as usize], b.mesh.positions[i as usize]);
        assert!(f.surface.distance(p) < 1e-9);
        let expect = f.surface.normal_at(p) * if f.outward { 1.0 } else { -1.0 };
        assert!(b.mesh.normals[k].distance(expect) < 1e-6);
    }
}

#[test]
fn l_path_with_centred_profile_keeps_area_times_length() {
    let b = run(&rect_profile(), &yz(), &l_path()).unwrap();
    assert!(b.mesh.is_closed());
    assert!((b.volume() - 8.0 * 50.0).abs() < 1e-9, "{}", b.volume());
    assert!(b.faces.iter().all(|f| matches!(f.surface, Surface::Plane { .. })));
}

#[test]
fn l_path_with_off_centre_profile_follows_the_centroid() {
    // Centroid at y = 3 on the inner side of the left turn: its trace is 27 along X, then 17 along Y.
    let b = run(&make(&[rect(1.0, -1.0, 5.0, 1.0)]), &yz(), &l_path()).unwrap();
    assert!(b.mesh.is_closed());
    assert!((b.volume() - 8.0 * 44.0).abs() < 1e-9, "{}", b.volume());
    // Outer side: y = -3, the trace is 33 then 23.
    let b = run(&make(&[rect(-5.0, -1.0, -1.0, 1.0)]), &yz(), &l_path()).unwrap();
    assert!(b.mesh.is_closed());
    assert!((b.volume() - 8.0 * 56.0).abs() < 1e-9, "{}", b.volume());
}

fn quarter_path() -> [Shape; 1] {
    [Shape::Arc { center: v2(0.0, 20.0), radius: 20.0, start: -FRAC_PI_2, sweep: FRAC_PI_2 }]
}

#[test]
fn quarter_arc_path_matches_pappus_up_to_faceting() {
    let b = run(&make(&[circle(0.0, 0.0, 2.0)]), &yz(), &quarter_path()).unwrap();
    assert!(b.mesh.is_closed());
    // The first chord leaves the tangent by half a chord angle, so the profile plane is not perpendicular to d_0:
    // the true cross-section is the profile foreshortened by cos(that angle), and V = A cos(a) L exactly.
    let half_chord = FRAC_PI_2 / arc_steps(20.0, FRAC_PI_2, TOL) as f64 / 2.0;
    let exact = facet_area(2.0) * half_chord.cos() * facet_length(20.0, FRAC_PI_2);
    close(b.volume(), exact, 1e-9);
    // The planned 0.5 % is below the faceting itself: a 32-gon at tol 0.01 loses (1 - sin(a) / a) = 0.64 % of the area (a = pi / 16).
    let pappus = PI * 4.0 * FRAC_PI_2 * 20.0;
    let deficit = 1.0 - b.volume() / pappus;
    assert!(deficit > 0.0 && deficit < 0.0075, "{deficit}");
    assert!(b.faces.iter().any(|f| f.surface == Surface::Faceted));
    assert_eq!(count(&b, |s| matches!(s, Surface::Plane { .. })), 2);
}

#[test]
fn closed_square_path_has_no_caps() {
    let sq = [line((0.0, 0.0), (40.0, 0.0)), line((40.0, 0.0), (40.0, 40.0)), line((40.0, 40.0), (0.0, 40.0)), line((0.0, 40.0), (0.0, 0.0))];
    let b = run(&rect_profile(), &yz(), &sq).unwrap();
    assert!(b.mesh.is_closed());
    assert!((b.volume() - 8.0 * 160.0).abs() < 1e-9, "{}", b.volume());
    // Four rings of the profile, no extra cap vertices.
    assert_eq!(b.mesh.positions.len(), 4 * 4);
}

#[test]
fn closed_circular_path_is_a_torus() {
    let plane = Plane3 { origin: DVec3::new(20.0, 0.0, 0.0), x: DVec3::Z, y: DVec3::X };
    let b = run(&make(&[circle(0.0, 0.0, 2.0)]), &plane, &[circle(0.0, 0.0, 20.0)]).unwrap();
    assert!(b.mesh.is_closed());
    let n = 2 * arc_steps(20.0, PI, TOL);
    let perimeter = facet_length(20.0, PI) * 2.0;
    assert!(n >= 4);
    let half_chord = PI / arc_steps(20.0, PI, TOL) as f64 / 2.0;
    close(b.volume(), facet_area(2.0) * half_chord.cos() * perimeter, 1e-9);
    let torus = 2.0 * PI * PI * 20.0 * 4.0;
    let deficit = 1.0 - b.volume() / torus;
    assert!(deficit > 0.0 && deficit < 0.0075, "{deficit}");
}

#[test]
fn tube_along_the_l_path() {
    let tube = make(&[circle(0.0, 0.0, 2.0), circle(0.0, 0.0, 1.0)]);
    let b = run(&tube, &yz(), &l_path()).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), (facet_area(2.0) - facet_area(1.0)) * 50.0, 1e-9);
}

#[test]
fn bad_inputs_are_rejected() {
    let along = Plane3::XY;
    let err = run(&rect_profile(), &along, &[line((0.0, 0.0), (30.0, 0.0))]).err();
    assert_eq!(err, Some(ModelError::Invalid("profile plane must not contain the path direction")));
    let tight = [Shape::Arc { center: v2(0.0, 1.0), radius: 1.0, start: -FRAC_PI_2, sweep: FRAC_PI_2 }];
    let err = run(&rect_profile(), &yz(), &tight).err();
    assert!(matches!(err, Some(ModelError::Invalid(m)) if m.starts_with("sweep folds over itself")), "{err:?}");
    assert_eq!(sweep(&rect_profile(), &yz(), &[], false, TOL).err(), Some(ModelError::Invalid("sweep path is empty")));
    let back = [line((0.0, 0.0), (10.0, 0.0)), line((10.0, 0.0), (0.0, 0.0001))];
    assert!(run(&rect_profile(), &yz(), &back).is_err());
}

#[test]
fn union_with_a_box_stays_closed() {
    let pipe = run(&make(&[circle(0.0, 0.0, 2.0)]), &yz(), &quarter_path()).unwrap();
    let u = boolean(&pipe, &box_body([-3.0, -5.0, -5.0], [5.0, 5.0, 5.0]), BoolOp::Union).unwrap();
    assert!(u.mesh.is_closed());
    assert!(u.volume() > pipe.volume() && u.volume() > 800.0);
    assert!(u.faces.iter().any(|f| f.surface == Surface::Faceted));
}

#[test]
fn edges_next_to_faceted_faces_are_other() {
    let tube = make(&[circle(0.0, 0.0, 2.0), circle(0.0, 0.0, 1.0)]);
    let b = run(&tube, &yz(), &quarter_path()).unwrap();
    let edges = b.edges();
    assert!(!edges.is_empty());
    let faceted = |f: u32| b.faces[f as usize].surface == Surface::Faceted;
    assert!(edges.iter().any(|e| e.faces.iter().any(|&f| faceted(f))));
    for e in edges.iter().filter(|e| e.faces.iter().any(|&f| faceted(f))) {
        assert_eq!(e.geom, EdgeGeom::Other);
    }
}

#[test]
fn faceted_normals_are_unit_and_outward() {
    let b = run(&make(&[circle(0.0, 0.0, 2.0)]), &yz(), &quarter_path()).unwrap();
    assert_eq!(b.mesh.normals.len(), b.mesh.indices.len());
    for (t, tri) in b.mesh.indices.as_chunks::<3>().0.iter().enumerate() {
        let [p, q, r] = tri.map(|i| b.mesh.positions[i as usize]);
        let face_n = (q - p).cross(r - p);
        for k in 0..3 {
            let n = b.mesh.normals[3 * t + k];
            assert!((n.length() - 1.0).abs() < 1e-9);
            assert!(n.dot(face_n) > 0.0);
        }
    }
}
