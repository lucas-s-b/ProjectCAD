use super::*;
use cad_modeler::{BodyFace, TriMesh};
use glam::DVec3;

fn plane() -> BodyFace {
    BodyFace { surface: Surface::Plane { origin: DVec3::ZERO, normal: DVec3::Z }, outward: true }
}

fn body(faces: Vec<BodyFace>) -> Body {
    Body { mesh: TriMesh::default(), tri_face: vec![], faces }
}

#[test]
fn accuracy_of_body() {
    assert_eq!(Accuracy::of_body(&body(vec![plane(), plane()])), Accuracy::Exact);
    let cyl = BodyFace { surface: Surface::Cylinder { origin: DVec3::ZERO, axis: DVec3::Z, radius: 5.0 }, outward: true };
    assert_eq!(Accuracy::of_body(&body(vec![plane(), cyl])), Accuracy::Approximate(TESSELLATION));
}

#[test]
fn accuracy_texts() {
    assert_eq!(Accuracy::Exact.to_string(), "exact");
    assert_eq!(Accuracy::Approximate(TESSELLATION).to_string(), "approximate (tessellation)");
    assert_eq!(Accuracy::Validated("cantilever beam").to_string(), "validated against cantilever beam");
}
