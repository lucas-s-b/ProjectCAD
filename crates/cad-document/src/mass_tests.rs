use super::*;
use crate::rebuild_pattern::tests::{extrude, push, put, rect};
use crate::*;
use cad_geometry::DVec3;

fn rel(a: f64, b: f64, r: f64) {
    assert!((a - b).abs() <= r * b.abs(), "{a} vs {b}");
}

fn near3(a: DVec3, b: DVec3, tol: f64) {
    assert!(a.distance(b) <= tol, "{a} vs {b}");
}

/// Body 1: the box 40 x 20 x 10 at the origin.
fn boxed() -> Document {
    let mut d = Document::new("mass");
    put(&mut d, 1, 0, rect(0.0, 0.0, 40.0, 20.0));
    push(&mut d, extrude(1, 1, "10 mm", BodyOp::New));
    d
}

/// Body 2: the default shaft (a turned, curved part) at x 100.
fn with_shaft(d: &mut Document) {
    let kind = FeatureKind::Part { def: PartDef::defaults()[1].clone(), sketch: 0, x: "100 mm".into(), y: "0".into(), op: BodyOp::New, target: Target::Auto };
    push(d, Feature { id: 2, name: "shaft".into(), suppressed: false, kind });
}

fn comp(id: ComponentId, body: FeatureId, x: &str, material: &str) -> Component {
    let mut meta = std::collections::BTreeMap::new();
    if !material.is_empty() {
        meta.insert("material".to_string(), material.to_string());
    }
    Component { id, name: format!("c{id}"), body, at: [x.into(), "0".into(), "0".into()], angles: ["0".into(), "0".into(), "0".into()], meta, grounded: false }
}

fn add(d: &mut Document, c: Component) {
    d.apply(Command::AddComponent(c)).unwrap();
}

fn report(d: &Document) -> MassReport {
    d.mass(&Rebuilder::default().rebuild(d))
}

#[test]
fn bodies_at_the_assumed_density() {
    let rep = report(&boxed());
    assert_eq!(rep.bodies.len(), 1);
    let (id, row) = &rep.bodies[0];
    assert_eq!(*id, 1);
    rel(row.props.mass, 8.0, 1e-9);
    near3(row.props.com, DVec3::new(20.0, 10.0, 5.0), 1e-9);
    assert_eq!(row.density, 1.0);
    assert!(row.assumed);
    assert_eq!(row.accuracy, Accuracy::Exact);
    assert!(rep.components.is_empty());
    assert_eq!(rep.assembly, None);
}

#[test]
fn two_steel_components() {
    let mut d = boxed();
    add(&mut d, comp(1, 1, "0", "Steel"));
    add(&mut d, comp(2, 1, "50 mm", "Steel"));
    let rep = report(&d);
    assert_eq!(rep.components.len(), 2);
    for (_, r) in &rep.components {
        assert!(!r.assumed);
        assert_eq!(r.density, 7.85);
        rel(r.props.mass, 62.8, 1e-9);
    }
    near3(rep.components[1].1.props.com, DVec3::new(70.0, 10.0, 5.0), 1e-9);
    let a = rep.assembly.unwrap();
    rel(a.props.mass, 125.6, 1e-9);
    rel(a.props.volume, 16000.0, 1e-9);
    assert!((a.density - 7.85).abs() <= 1e-12);
    near3(a.props.com, DVec3::new(45.0, 10.0, 5.0), 1e-9);
    assert!(!a.assumed);
    assert_eq!(a.accuracy, Accuracy::Exact);
    let want = [5233.333333333333, 96293.33333333333, 99433.33333333333];
    let i = a.props.inertia;
    for (got, w) in [i.x_axis.x, i.y_axis.y, i.z_axis.z].into_iter().zip(want) {
        assert!((got - w).abs() <= 1e-5, "{got} vs {w}");
    }
    for (got, w) in a.props.principal().0.into_iter().zip(want) {
        assert!((got - w).abs() <= 1e-5, "{got} vs {w}");
    }
}

#[test]
fn unknown_material_and_curved_body() {
    let mut d = boxed();
    with_shaft(&mut d);
    add(&mut d, comp(1, 1, "0", "Unobtainium"));
    add(&mut d, comp(2, 1, "50 mm", "Steel"));
    add(&mut d, comp(3, 2, "0", ""));
    let rep = report(&d);
    let get = |id| &rep.components.iter().find(|(i, _)| *i == id).unwrap().1;
    assert!(get(1).assumed);
    assert_eq!(get(1).density, 1.0);
    assert_eq!(get(3).accuracy, Accuracy::Approximate("tessellation"));
    let a = rep.assembly.as_ref().unwrap();
    assert!(a.assumed);
    assert_eq!(a.accuracy, Accuracy::Approximate("tessellation"));
    rel(a.props.mass, get(1).props.mass + get(2).props.mass + get(3).props.mass, 1e-9);
}

#[test]
fn failed_component_is_left_out() {
    let mut d = boxed();
    add(&mut d, comp(1, 1, "0", "Steel"));
    add(&mut d, comp(2, 1, "1 deg", "Steel"));
    let r = Rebuilder::default().rebuild(&d);
    assert!(r.components.iter().find(|(i, _)| *i == 2).unwrap().1.is_err());
    let rep = d.mass(&r);
    assert_eq!(rep.components.len(), 1);
    assert_eq!(rep.components[0].0, 1);
}
