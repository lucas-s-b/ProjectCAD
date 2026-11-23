use super::*;
use cad_document::{Document, History, Rebuilder};

fn ones() -> NextIds {
    NextIds { component: 1, constraint: 1, entity: 1, feature: 1, group: 1, joint: 1, layer: 1, sketch: 1 }
}

fn volume(flange: &BoltCircle) -> f64 {
    let mut d = Document::new("t");
    let cmds = flange.commands(&d.next_ids(), 0);
    History::default().execute(&mut d, cmds).unwrap();
    assert_eq!(d.len(), 1 + flange.count as usize);
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.bodies.len(), 1);
    r.bodies[0].1.volume()
}

#[test]
fn default_flange_has_the_expected_volume() {
    let v = volume(&BoltCircle::default());
    assert!((v - 110_081.4).abs() < 1_100.0, "{v}");
    let v = volume(&BoltCircle { count: 8, ..Default::default() });
    assert!((v - 109_076.1).abs() < 1_090.0, "{v}");
}

#[test]
fn numbering_starts_at_next() {
    let next = NextIds { entity: 10, feature: 4, ..ones() };
    let cmds = BoltCircle::default().commands(&next, 3);
    assert_eq!(cmds.len(), 14);
    for (i, c) in cmds[..7].iter().enumerate() {
        assert!(matches!(c, Command::Add(e) if e.id == 10 + i as u64), "{i}");
    }
    let hole = |i: usize| match &cmds[i] {
        Command::Add(e) => match e.shape {
            Shape::Circle { center, radius } => (e.id, center, radius),
            _ => panic!("not a circle"),
        },
        _ => panic!("not an Add"),
    };
    let (id, c, r) = hole(1);
    assert!(id == 11 && (c - DVec2::new(40.0, 0.0)).length() < 1e-6 && r == 4.0);
    let (id, c, r) = hole(2);
    assert!(id == 12 && (c - DVec2::new(20.0, 34.641016)).length() < 1e-6 && r == 4.0);
    let Command::AddFeature { index: 3, feature } = &cmds[7] else { panic!("command 8") };
    assert!(feature.id == 4 && matches!(&feature.kind, FeatureKind::Extrude { profile, .. } if profile == &[10]));
    for i in 0..6 {
        let Command::AddFeature { index, feature } = &cmds[8 + i] else { panic!("command {}", 9 + i) };
        assert!(*index == 4 + i && feature.id == 5 + i as u64);
        assert!(matches!(&feature.kind, FeatureKind::Extrude { profile, op: BodyOp::Cut, .. } if profile == &[11 + i as u64]));
    }
}

#[test]
fn check_rejects_bad_sizes() {
    assert!(BoltCircle::default().check().is_ok());
    let d = BoltCircle::default;
    for bad in [
        BoltCircle { count: 0, ..d() },
        BoltCircle { count: 361, ..d() },
        BoltCircle { hole_diameter: 45.0, ..d() },
        BoltCircle { thickness: -1.0, ..d() },
        BoltCircle { bolt_radius: f64::NAN, ..d() },
        BoltCircle { count: 32, ..d() },
    ] {
        assert!(bad.check().is_err(), "{bad:?}");
    }
    assert!(BoltCircle { count: 31, ..d() }.check().is_ok());
}

#[test]
fn args_parse_and_reject() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let a = parse_args(&s(&["--count", "8", "--save", "f.cadproj"])).unwrap();
    assert_eq!(a.flange, BoltCircle { count: 8, ..Default::default() });
    assert_eq!((a.save, a.app), (Some("f.cadproj".into()), None));
    assert_eq!(parse_args(&[]).unwrap().flange, BoltCircle::default());
    assert_eq!(parse_args(&s(&["--count"])).unwrap_err(), "missing value for --count");
    assert_eq!(parse_args(&s(&["--size", "3"])).unwrap_err(), "unknown argument --size");
    assert!(parse_args(&s(&["--count", "x"])).unwrap_err().starts_with("--count:"));
}
