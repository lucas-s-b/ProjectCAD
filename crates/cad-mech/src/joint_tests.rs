use super::*;
use crate::{Nut, metric};
use cad_geometry::DAffine3;
use cad_modeler::Plane3;
use std::f64::consts::FRAC_PI_2;

fn near3(a: DVec3, b: DVec3, tol: f64) {
    assert!(a.distance(b) <= tol, "{a} vs {b}");
}

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

#[test]
fn frames_from_axes() {
    let f = frame(v(1.0, 2.0, 3.0), v(0.0, 0.0, 2.0)).unwrap();
    near3(f.transform_point3(v(1.0, 0.0, 0.0)), v(2.0, 2.0, 3.0), 1e-12);
    near3(f.transform_point3(v(0.0, 1.0, 0.0)), v(1.0, 3.0, 3.0), 1e-12);
    let f = frame(DVec3::ZERO, DVec3::X).unwrap();
    near3(f.transform_point3(v(1.0, 0.0, 0.0)), v(0.0, 1.0, 0.0), 1e-12);
    near3(f.transform_point3(v(0.0, 1.0, 0.0)), v(0.0, 0.0, 1.0), 1e-12);
    near3(f.transform_point3(v(0.0, 0.0, 1.0)), v(1.0, 0.0, 0.0), 1e-12);
    let f = frame(DVec3::ZERO, -DVec3::Z).unwrap();
    near3(f.transform_point3(v(0.0, 1.0, 0.0)), v(0.0, -1.0, 0.0), 1e-12);
    near3(f.transform_point3(v(0.0, 0.0, 1.0)), v(0.0, 0.0, -1.0), 1e-12);
    assert!(frame(DVec3::ZERO, DVec3::ZERO).is_none());
}

#[test]
fn mates() {
    let stack = mate(frame(v(5.0, 10.0, 5.0), DVec3::Z).unwrap(), DAffine3::IDENTITY, frame(v(5.0, 10.0, 0.0), -DVec3::Z).unwrap());
    near3(stack.transform_point3(DVec3::ZERO), v(0.0, 0.0, 5.0), 1e-9);
    near3(stack.transform_point3(v(10.0, 20.0, 0.0)), v(10.0, 20.0, 5.0), 1e-9);

    let turn = mate(frame(v(10.0, 0.0, 0.0), DVec3::Z).unwrap(), DAffine3::from_rotation_z(FRAC_PI_2), frame(DVec3::ZERO, -DVec3::Z).unwrap());
    near3(turn.transform_point3(DVec3::ZERO), v(10.0, 0.0, 0.0), 1e-9);
    near3(turn.transform_point3(v(10.0, 0.0, 0.0)), v(10.0, 10.0, 0.0), 1e-9);

    let slide = mate(frame(v(10.0, 0.0, 0.0), DVec3::X).unwrap(), DAffine3::from_translation(v(0.0, 0.0, 2.0)), frame(DVec3::ZERO, -DVec3::X).unwrap());
    near3(slide.transform_point3(v(1.0, 2.0, 3.0)), v(13.0, 2.0, 3.0), 1e-9);
}

#[test]
fn nut_face_frames() {
    let b = Nut { size: metric("M8").unwrap(), modelled: false }.body(&Plane3::XY, 0.001).unwrap();
    assert!((0..b.faces.len()).all(|i| face_frame(&b, i).is_some()));
    assert!(face_frame(&b, b.faces.len()).is_none());
    let find = |axis: DVec3| (0..b.faces.len()).filter_map(|i| face_frame(&b, i)).find(|(_, a)| a.distance(axis) <= 1e-9).unwrap().0;
    near3(find(DVec3::Z), v(0.0, 0.0, 6.8), 1e-6);
    near3(find(-DVec3::Z), v(0.0, 0.0, 0.0), 1e-6);
    near3(find(DVec3::X), v(6.5, 0.0, 3.4), 1e-6);
    let i = (0..b.faces.len()).find(|&i| matches!(b.faces[i].surface, Surface::Cylinder { .. })).unwrap();
    let (o, a) = face_frame(&b, i).unwrap();
    near3(o, v(0.0, 0.0, 3.4), 1e-6);
    assert!(a.cross(DVec3::Z).length() <= 1e-12);
    assert!((a.length() - 1.0).abs() <= 1e-12);
}
