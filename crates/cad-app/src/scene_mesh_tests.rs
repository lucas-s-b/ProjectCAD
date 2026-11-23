use super::mesh_data;
use cad_document::{BodyOp, Command, Document, Entity, Feature, FeatureKind, Rebuilder, Target};
use cad_geometry::{DVec2, Shape};

#[test]
fn mesh_data_of_a_box() {
    let mut doc = Document::new("box");
    let id = doc.alloc_id();
    let shape = Shape::rect(DVec2::ZERO, DVec2::new(10.0, 20.0));
    doc.apply(Command::Add(Entity { id, name: String::new(), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
    let kind = FeatureKind::Extrude { profile: vec![id], distance: "5".into(), offset: "0".into(), taper: "0 deg".into(), op: BodyOp::New, flip: false, target: Target::Auto };
    let feature = Feature { id: doc.alloc_feature_id(), name: "box".into(), suppressed: false, kind };
    doc.apply(Command::AddFeature { index: 0, feature }).unwrap();
    let result = Rebuilder::default().rebuild(&doc);
    let data = mesh_data(&result.bodies[0].1);
    assert_eq!(data.tris.len(), 36);
    assert_eq!(data.lines.len(), 24);
    assert!(data.tris.iter().all(|v| v.color == [1.0; 4]));
    assert!(data.lines.iter().all(|v| v.normal == [0.0; 3]));
}
