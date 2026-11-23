use crate::rebuild_pattern::tests::push;
use crate::*;
use cad_geometry::DVec3;
use cad_mech::{Bolt, Gear, metric};

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs().max(1.0), "{a} vs {b}");
}

fn near3(a: DVec3, b: DVec3, tol: f64) {
    assert!(a.distance(b) <= tol, "{a} vs {b}");
}

fn part(id: FeatureId, def: PartDef, sketch: SketchId, x: &str) -> Feature {
    let kind = FeatureKind::Part { def, sketch, x: x.into(), y: "0".into(), op: BodyOp::New, target: Target::Auto };
    Feature { id, name: format!("part{id}"), suppressed: false, kind }
}

fn gear(teeth: &str) -> PartDef {
    PartDef::Gear { module: "2 mm".into(), teeth: teeth.into(), pressure: "20 deg".into(), width: "10 mm".into(), bore: "8 mm".into() }
}

fn bolt(size: &str, length: &str) -> PartDef {
    PartDef::Bolt { size: size.into(), length: length.into(), modelled: false }
}

fn rebuilt(d: &Document) -> RebuildResult {
    Rebuilder::default().rebuild(d)
}

fn add(d: &mut Document, f: Feature) -> Result<(), DocError> {
    let index = d.features().len();
    d.apply(Command::AddFeature { index, feature: f }).map(drop)
}

#[test]
fn gear_feature_builds_the_cad_mech_solid() {
    let mut d = Document::new("g");
    push(&mut d, part(1, gear("20"), 0, "0"));
    let r = rebuilt(&d);
    assert_eq!(r.bodies.len(), 1);
    assert_eq!(r.bodies[0].0, 1);
    assert_eq!(r.states[&1], FeatureState::Ok);
    let g = Gear { module: 2.0, teeth: 20, pressure: 20f64.to_radians(), width: 10.0, bore: 8.0 };
    let expected = g.body(&Plane3::XY, BODY_TOLERANCE).unwrap().volume();
    let v = r.bodies[0].1.volume();
    close(v, expected, 1e-12);
    close(v, 11755.14129241726, 1e-3);
    assert_eq!(d.part(&gear("20")).unwrap().designation(), "Spur gear m2 z20 b10");
}

#[test]
fn parameters_drive_parts() {
    let mut d = Document::new("g");
    d.apply(Command::SetParam { name: "teeth".into(), expr: Some("24".into()) }).unwrap();
    push(&mut d, part(1, gear("teeth"), 0, "0"));
    let mut rb = Rebuilder::default();
    close(rb.rebuild(&d).bodies[0].1.volume(), 17247.18256226141, 1e-3);
    assert_eq!(d.features_using_param("teeth"), vec![1]);
    d.apply(Command::SetParam { name: "teeth".into(), expr: Some("30".into()) }).unwrap();
    close(rb.rebuild(&d).bodies[0].1.volume(), 27383.42322872711, 1e-3);
    d.apply(Command::SetParam { name: "teeth".into(), expr: Some("20.5".into()) }).unwrap();
    let r = rb.rebuild(&d);
    assert_eq!(r.states[&1], FeatureState::Error("teeth must be a whole number".into()));
    assert!(r.bodies.is_empty());
}

#[test]
fn parts_sit_on_their_sketch() {
    let mut d = Document::new("g");
    let plane = PlaneDef::Principal { base: BasePlane::XY, offset: "100 mm".into() };
    d.apply(Command::AddSketch(Sketch { id: 1, name: "top".into(), plane, level: None })).unwrap();
    push(&mut d, part(1, bolt("M8", "30 mm"), 1, "60 mm"));
    let r = rebuilt(&d);
    assert_eq!(r.states[&1], FeatureState::Ok);
    let (lo, hi) = r.bodies[0].1.bbox();
    near3(lo, DVec3::new(53.5, -7.505553499465135, 70.0), 1e-6);
    near3(hi, DVec3::new(66.5, 7.505553499465135, 105.3), 1e-6);
    assert_eq!(d.part(&bolt("M8", "30 mm")).unwrap().thread(), Some("M8x1.25".to_string()));
}

#[test]
fn patterns_copy_parts() {
    let mut d = Document::new("g");
    push(&mut d, part(1, bolt("M8", "30 mm"), 0, "30 mm"));
    let kind = FeatureKind::CircularPattern {
        source: 1,
        count: "4".into(),
        angle: "360 deg".into(),
        axis: Axis3::Z,
        cx: "0".into(),
        cy: "0".into(),
        cz: "0".into(),
    };
    push(&mut d, Feature { id: 2, name: "ring".into(), suppressed: false, kind });
    let r = rebuilt(&d);
    assert_eq!(r.states[&1], FeatureState::Ok);
    assert_eq!(r.states[&2], FeatureState::Ok);
    assert_eq!(r.bodies.len(), 2);
    let at = Plane3 { origin: DVec3::new(30.0, 0.0, 0.0), ..Plane3::XY };
    let one = cad_mech::Part::Bolt(Bolt { size: metric("M8").unwrap(), length: 30.0, modelled: false }).body(&at, BODY_TOLERANCE).unwrap().volume();
    close(r.total_volume(), 4.0 * one, 1e-6);
}

#[test]
fn part_checks() {
    let mut d = Document::new("g");
    let plane = PlaneDef::Principal { base: BasePlane::XY, offset: "0".into() };
    d.apply(Command::AddSketch(Sketch { id: 1, name: "s".into(), plane, level: None })).unwrap();
    let nut = || PartDef::Nut { size: "M8".into(), modelled: false };
    push(&mut d, part(1, nut(), 1, "0"));
    assert!(matches!(d.apply(Command::RemoveSketch(1)), Err(DocError::SketchInUse(1))));
    assert!(matches!(add(&mut d, part(2, nut(), 9, "0")), Err(DocError::SketchNotFound(9))));
    assert!(matches!(add(&mut d, part(3, bolt("M7", "30 mm"), 0, "0")), Err(DocError::BadFeature("unknown thread size"))));
    let empty = PartDef::Shaft { steps: Vec::new(), chamfer: "0".into() };
    assert!(matches!(add(&mut d, part(4, empty, 0, "0")), Err(DocError::BadFeature("shaft needs at least one step"))));
    assert!(add(&mut d, part(5, bolt("M8", "30 +"), 0, "0")).is_err());
    d.apply(Command::RemoveFeature(1)).unwrap();
    assert!(d.apply(Command::RemoveSketch(1)).is_ok());
}

#[test]
fn invalid_parts_report_why() {
    let mut d = Document::new("g");
    let spring = PartDef::Spring { wire: "2 mm".into(), mean: "20 mm".into(), pitch: "1 mm".into(), turns: "6".into() };
    push(&mut d, part(1, spring, 0, "0"));
    let r = rebuilt(&d);
    assert_eq!(r.states[&1], FeatureState::Error("spring pitch must be larger than the wire diameter".into()));
    assert!(r.bodies.is_empty());
}

#[test]
fn every_default_part_builds_and_saves() {
    let mut d = Document::new("g");
    let defaults = PartDef::defaults();
    assert_eq!(defaults.iter().map(PartDef::label).collect::<Vec<_>>(), ["Gear", "Shaft", "Spring", "Bolt", "Nut", "Bearing"]);
    for (i, def) in defaults.into_iter().enumerate() {
        push(&mut d, part(i as u64 + 1, def, 0, &format!("{} mm", 100 * i)));
    }
    let r = rebuilt(&d);
    assert_eq!(r.bodies.len(), 6);
    assert!(r.states.values().all(|s| *s == FeatureState::Ok), "{:?}", r.states);
    let v: Vec<f64> = r.bodies.iter().map(|(_, b)| b.volume()).collect();
    close(v[0], 11755.14129241726, 1e-3);
    close(v[1], 19372.340134589296, 1e-9);
    close(v[2], 1173.7381100536138, 1e-6);
    close(v[3], 2283.6634278928223, 1e-2);
    close(v[4], 653.4311133185073, 1e-2);
    close(v[5], 13146.684979704914, 1e-2);
    let p = std::env::temp_dir().join(format!("cadtest_{}_parts.cadproj", std::process::id()));
    d.save(&p).unwrap();
    let loaded = Document::load(&p).unwrap();
    std::fs::remove_file(&p).ok();
    assert_eq!(loaded.features(), d.features());
    assert_eq!(FORMAT_VERSION, 27);
}

#[test]
fn modelled_threads_and_bearings() {
    let mut d = Document::new("g");
    push(&mut d, part(1, PartDef::Bolt { size: "M8".into(), length: "30 mm".into(), modelled: true }, 0, "0"));
    push(&mut d, part(2, PartDef::Nut { size: "M8".into(), modelled: true }, 0, "100 mm"));
    push(&mut d, part(3, PartDef::Bearing { size: "6204".into() }, 0, "200 mm"));
    let r = rebuilt(&d);
    assert!(r.states.values().all(|s| *s == FeatureState::Ok), "{:?}", r.states);
    let v: Vec<f64> = r.bodies.iter().map(|(_, b)| b.volume()).collect();
    close(v[0], 1985.8667876362092, 1e-9);
    close(v[1], 714.7977356403367, 1e-5);
    close(v[2], 13146.684979704914, 1e-2);
    assert_eq!(d.part(&PartDef::Bearing { size: "6204".into() }).unwrap().designation(), "Ball bearing 6204 20x47x14");
    assert!(matches!(add(&mut d, part(4, PartDef::Bearing { size: "6299".into() }, 0, "0")), Err(DocError::BadFeature("unknown bearing size"))));
}

#[test]
fn old_bolts_load_cosmetic() {
    let bolt: PartDef = serde_json::from_str(r#"{"Bolt":{"size":"M8","length":"30 mm"}}"#).unwrap();
    assert_eq!(bolt, PartDef::Bolt { size: "M8".into(), length: "30 mm".into(), modelled: false });
    let nut: PartDef = serde_json::from_str(r#"{"Nut":{"size":"M8"}}"#).unwrap();
    assert_eq!(nut, PartDef::Nut { size: "M8".into(), modelled: false });
}
