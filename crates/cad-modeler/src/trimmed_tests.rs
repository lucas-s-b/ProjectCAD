use super::*;
use crate::fixtures::*;
use crate::{BoolOp, Plane3, boolean, extrude};

/// V - E + F - (L - F), with L the total number of loops.
fn euler(t: &TrimmedBody) -> i64 {
    let loops: usize = t.faces.iter().map(|f| f.loops.len()).sum();
    t.vertices.len() as i64 - t.edges.len() as i64 + t.faces.len() as i64 - (loops as i64 - t.faces.len() as i64)
}

/// Per edge: (forward uses, backward uses).
fn uses(t: &TrimmedBody) -> Vec<(u32, u32)> {
    let mut out = vec![(0, 0); t.edges.len()];
    for &(e, fwd) in t.faces.iter().flat_map(|f| f.loops.iter().flatten()) {
        if fwd {
            out[e as usize].0 += 1;
        } else {
            out[e as usize].1 += 1;
        }
    }
    out
}

fn counts(t: &TrimmedBody) -> (usize, usize, usize) {
    (t.faces.len(), t.edges.len(), t.vertices.len())
}

#[test]
fn box_has_six_faces_twelve_lines_eight_vertices() {
    let t = TrimmedBody::from_body(&box_body([0.0; 3], [40.0, 20.0, 10.0])).unwrap();
    assert_eq!(counts(&t), (6, 12, 8));
    assert!(t.edges.iter().all(|e| matches!(e.geom, EdgeGeom::Line { .. })));
    assert!(t.faces.iter().all(|f| f.loops.len() == 1 && f.loops[0].len() == 4 && f.outer == Some(0)));
    assert_eq!(euler(&t), 2);
}

#[test]
fn cylinder_has_two_closed_circles_and_no_seam() {
    let t = TrimmedBody::from_body(&cylinder_body(0.0, 0.0, 5.0, 0.0, 10.0)).unwrap();
    assert_eq!(counts(&t), (3, 2, 2));
    for e in &t.edges {
        assert_eq!(e.vertices[0], e.vertices[1]);
        let EdgeGeom::Circle { radius, .. } = e.geom else { panic!("{:?}", e.geom) };
        assert_eq!(radius, 5.0);
    }
    let side = t.faces.iter().find(|f| matches!(f.surface, Surface::Cylinder { .. })).unwrap();
    assert!(side.loops.len() == 2 && side.loops.iter().all(|l| l.len() == 1) && side.outer.is_none());
    assert_eq!(euler(&t), 2);
}

#[test]
fn plate_with_hole_has_genus_one() {
    let bodies = [
        boolean(&box_body([0.0; 3], [40.0, 20.0, 10.0]), &cylinder_body(20.0, 10.0, 5.0, -1.0, 11.0), BoolOp::Difference).unwrap(),
        boolean(&box_body([0.0; 3], [40.0, 20.0, 10.0]), &cylinder_body(20.0, 10.0, 4.0, 0.0, 10.0), BoolOp::Difference).unwrap(),
    ];
    for b in bodies {
        let t = TrimmedBody::from_body(&b).unwrap();
        assert_eq!(counts(&t), (7, 14, 10));
        assert_eq!(euler(&t), 0);
        let top = t
            .faces
            .iter()
            .find(|f| matches!(f.surface, Surface::Plane { normal, .. } if normal.z.abs() > 0.999) && f.surface.distance(DVec3::new(0.0, 0.0, 10.0)) < 1e-9)
            .unwrap();
        assert_eq!(top.loops.len(), 2);
        assert_eq!(top.loops[top.outer.unwrap()].len(), 4);
    }
}

#[test]
fn every_edge_is_used_once_each_way() {
    let bodies = [
        boolean(&box_body([0.0; 3], [40.0, 20.0, 10.0]), &cylinder_body(20.0, 10.0, 5.0, -1.0, 11.0), BoolOp::Difference).unwrap(),
        d_flat(),
        oblique_cut(),
    ];
    for b in bodies {
        let t = TrimmedBody::from_body(&b).unwrap();
        assert!(uses(&t).iter().all(|&u| u == (1, 1)), "{:?}", uses(&t));
    }
}

#[test]
fn d_flat_vertices_are_snapped() {
    let t = TrimmedBody::from_body(&d_flat()).unwrap();
    assert_eq!(counts(&t), (4, 6, 4));
    assert_eq!(euler(&t), 2);
    for v in &t.vertices {
        assert!((v.x - 3.0).abs() < 1e-6 && (v.y.abs() - 4.0).abs() < 1e-6 && (v.z.abs() < 1e-6 || (v.z - 10.0).abs() < 1e-6), "{v:?}");
    }
}

#[test]
fn oblique_cut_has_an_ellipse() {
    let b = oblique_cut();
    let t = TrimmedBody::from_body(&b).unwrap();
    assert_eq!(counts(&t), (3, 2, 2));
    let e = t.edges.iter().find(|e| matches!(e.geom, EdgeGeom::Ellipse { .. })).unwrap();
    let v = t.vertices[e.vertices[0] as usize];
    let planes: Vec<_> = t.faces.iter().filter(|f| matches!(f.surface, Surface::Plane { normal, .. } if normal.z.abs() < 0.9)).collect();
    assert_eq!(planes.len(), 1);
    assert!(planes[0].surface.distance(v) < 1e-6);
    let cyl = t.faces.iter().find(|f| matches!(f.surface, Surface::Cylinder { .. })).unwrap();
    assert!(cyl.surface.distance(v) < 1e-6);
}

#[test]
fn other_surfaces_and_curves_are_refused() {
    let mut b = box_body([0.0; 3], [40.0, 20.0, 10.0]);
    b.faces[0].surface = Surface::Faceted;
    assert_eq!(TrimmedBody::from_body(&b), Err(NOT_ANALYTIC));
    let across = Plane3 { origin: DVec3::new(-10.0, 0.0, 0.0), x: DVec3::Y, y: DVec3::Z };
    let beam = Body::from_brep(&extrude(&make(&[circle(0.0, 0.0, 3.0)]), &across, 20.0).unwrap(), 0.01).unwrap();
    let u = boolean(&cylinder_body(0.0, 0.0, 5.0, -10.0, 10.0), &beam, BoolOp::Union).unwrap();
    assert_eq!(TrimmedBody::from_body(&u), Err(OTHER_EDGE));
}
