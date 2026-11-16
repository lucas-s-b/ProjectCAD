use crate::document::*;
use crate::units::Settings;
use cad_geometry::{DVec2, Shape};


fn circle(r: f64) -> Shape {
    Shape::Circle { center: DVec2::ZERO, radius: r }
}
fn entity(id: EntityId, r: f64) -> Entity {
    Entity { id, name: format!("c{id}"), shape: circle(r), layer: 0, sketch: 0 }
}

#[test]
fn inverses_are_correct() {
    let mut d = Document::new("t");
    let inv = d.apply(Command::Add(entity(1, 1.0))).unwrap();
    assert_eq!(inv, Command::Remove(1));
    let inv = d.apply(Command::SetShape { id: 1, shape: circle(2.0) }).unwrap();
    assert_eq!(inv, Command::SetShape { id: 1, shape: circle(1.0) });
    let inv = d.apply(Command::Remove(1)).unwrap();
    assert_eq!(inv, Command::Add(Entity { id: 1, name: "c1".into(), shape: circle(2.0), layer: 0, sketch: 0 }));
    assert!(d.is_empty());
}

#[test]
fn rejects_invalid_and_leaves_document_unchanged() {
    let mut d = Document::new("t");
    d.apply(Command::Add(entity(1, 1.0))).unwrap();
    let rev = d.revision();
    assert!(matches!(d.apply(Command::Add(entity(2, 0.0))), Err(DocError::Geometry(_))));
    assert!(matches!(
        d.apply(Command::SetShape { id: 1, shape: circle(-1.0) }),
        Err(DocError::Geometry(_))
    ));
    assert_eq!(d.len(), 1);
    assert_eq!(d.get(1).unwrap().shape, circle(1.0));
    assert_eq!(d.revision(), rev);
}

#[test]
fn duplicate_and_missing_ids() {
    let mut d = Document::new("t");
    d.apply(Command::Add(entity(1, 1.0))).unwrap();
    assert!(matches!(d.apply(Command::Add(entity(1, 2.0))), Err(DocError::DuplicateId(1))));
    assert!(matches!(d.apply(Command::Remove(9)), Err(DocError::NotFound(9))));
    assert!(matches!(
        d.apply(Command::SetShape { id: 9, shape: circle(1.0) }),
        Err(DocError::NotFound(9))
    ));
}

fn layer(id: LayerId, name: &str) -> Layer {
    Layer::new(id, name)
}

#[test]
fn layer_commands_invert_and_roundtrip() {
    let mut d = Document::new("t");
    let l1 = layer(1, "A");
    assert_eq!(d.apply(Command::AddLayer(l1.clone())).unwrap(), Command::RemoveLayer(1));
    assert_eq!(d.alloc_layer_id(), 2);
    let mut l1b = l1.clone();
    l1b.visible = false;
    assert_eq!(d.apply(Command::SetLayer(l1b.clone())).unwrap(), Command::SetLayer(l1.clone()));
    assert_eq!(d.layer(1), Some(&l1b));
    d.apply(Command::Add(entity(1, 1.0))).unwrap();
    let inv = d.apply(Command::SetEntityLayer { id: 1, layer: 1 }).unwrap();
    assert_eq!(inv, Command::SetEntityLayer { id: 1, layer: 0 });
    d.apply(inv).unwrap();
    assert_eq!(d.apply(Command::RemoveLayer(1)).unwrap(), Command::AddLayer(l1b));
    assert_eq!(d.layers().count(), 1);
}

#[test]
fn layer_errors_leave_document_unchanged() {
    let mut d = Document::new("t");
    d.apply(Command::AddLayer(layer(1, "A"))).unwrap();
    d.apply(Command::Add(Entity { layer: 1, ..entity(1, 1.0) })).unwrap();
    let rev = d.revision();
    assert!(matches!(d.apply(Command::AddLayer(layer(1, "B"))), Err(DocError::LayerExists(1))));
    assert!(matches!(d.apply(Command::AddLayer(layer(2, "  "))), Err(DocError::EmptyLayerName)));
    assert!(matches!(d.apply(Command::RemoveLayer(0)), Err(DocError::ProtectedLayer)));
    assert!(matches!(d.apply(Command::RemoveLayer(7)), Err(DocError::LayerNotFound(7))));
    assert!(matches!(d.apply(Command::RemoveLayer(1)), Err(DocError::LayerInUse(1))));
    assert!(matches!(d.apply(Command::SetLayer(layer(7, "x"))), Err(DocError::LayerNotFound(7))));
    assert!(matches!(d.apply(Command::SetLayer(layer(1, ""))), Err(DocError::EmptyLayerName)));
    assert!(matches!(
        d.apply(Command::SetEntityLayer { id: 1, layer: 9 }),
        Err(DocError::LayerNotFound(9))
    ));
    assert!(matches!(
        d.apply(Command::Add(Entity { layer: 9, ..entity(2, 1.0) })),
        Err(DocError::LayerNotFound(9))
    ));
    assert_eq!((d.revision(), d.len(), d.layers().count()), (rev, 1, 2));
    assert_eq!(d.get(1).unwrap().layer, 1);
}

#[test]
fn set_settings_inverts_and_validates() {
    let mut d = Document::new("t");
    let s = Settings { units: crate::units::Units::Inch, precision: 4 };
    let inv = d.apply(Command::SetSettings(s)).unwrap();
    assert_eq!(inv, Command::SetSettings(Settings::default()));
    assert_eq!(*d.settings(), s);
    d.apply(inv).unwrap();
    assert_eq!(*d.settings(), Settings::default());
    let rev = d.revision();
    let bad = Settings { precision: 9, ..s };
    assert!(matches!(d.apply(Command::SetSettings(bad)), Err(DocError::BadPrecision)));
    assert_eq!(d.revision(), rev);
}

#[test]
fn add_bumps_next_id_and_revision_counts() {
    let mut d = Document::new("t");
    assert_eq!(d.revision(), 0);
    d.apply(Command::Add(entity(10, 1.0))).unwrap();
    assert_eq!(d.alloc_id(), 11);
    d.apply(Command::SetShape { id: 10, shape: circle(3.0) }).unwrap();
    d.apply(Command::Remove(10)).unwrap();
    assert_eq!(d.revision(), 3);
}

#[test]
fn line_weight_is_validated() {
    let mut d = Document::new("t");
    let with = |w: f64| Layer { line_weight: w, ..Layer::new(1, "A") };
    for w in [0.04, 2.01, f64::NAN, f64::INFINITY] {
        assert!(matches!(d.apply(Command::AddLayer(with(w))), Err(DocError::BadLineWeight)), "{w}");
    }
    d.apply(Command::AddLayer(with(0.05))).unwrap();
    assert!(matches!(d.apply(Command::SetLayer(with(3.0))), Err(DocError::BadLineWeight)));
    let l = Layer { line_type: crate::LineType::Dashed, ..with(2.0) };
    d.apply(Command::SetLayer(l.clone())).unwrap();
    assert_eq!(d.layer(1), Some(&l));
}
