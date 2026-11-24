use crate::mass::tests::boxed;
use crate::*;
use cad_geometry::DVec3;

/// The reference of the body's face that points along `n`.
fn face(r: &RebuildResult, n: DVec3) -> FaceRef {
    let body = &r.bodies[0].1;
    FaceRef::of(body, (0..body.faces.len()).find(|&i| FaceKey::of(&body.faces[i]).matches(&FaceKey::Plane(n))).unwrap()).unwrap()
}

/// 1000 N along +X on the +X face of the box, the -X face held, steel with Poisson's ratio 0 (an exact bar).
fn tension(r: &RebuildResult) -> Study {
    Study {
        material: "Custom".into(),
        poisson: "0".into(),
        fixed: vec![face(r, -DVec3::X)],
        loads: vec![Load { face: face(r, DVec3::X), kind: LoadKind::Force(["1000".into(), "0".into(), "0".into()]) }],
        ..Study::new("Study 1".into(), 1, "5 mm".into())
    }
}

#[test]
fn material_helpers() {
    let mut s = Study::new("Study 1".into(), 1, "5 mm".into());
    assert_eq!((s.material.as_str(), s.young.as_str(), s.poisson.as_str(), s.density.as_str()), ("Steel", "210000", "0.3", "7.85"));
    assert!(!s.gravity && s.fixed.is_empty() && s.loads.is_empty());
    assert!(s.set_material("aluminium"));
    assert_eq!((s.material.as_str(), s.young.as_str(), s.poisson.as_str(), s.density.as_str()), ("Aluminium", "69000", "0.33", "2.7"));
    let before = s.clone();
    assert!(!s.set_material("Unobtainium"));
    assert_eq!(s, before);
    s.custom_if_edited();
    assert_eq!(s.material, "Aluminium");
    s.poisson = " 0.33 ".into();
    s.custom_if_edited();
    assert_eq!(s.material, "Aluminium");
    s.poisson = "0".into();
    s.custom_if_edited();
    assert_eq!(s.material, "Custom");
}

#[test]
fn yield_strength_helpers() {
    let mut s = Study::new("Study 1".into(), 1, "5 mm".into());
    assert_eq!(s.yield_strength, "250");
    assert!(s.set_material("aluminium"));
    assert_eq!(s.yield_strength, "276");
    s.yield_strength = "".into();
    s.custom_if_edited();
    assert_eq!(s.material, "Aluminium");
    s.yield_strength = " 276 ".into();
    s.custom_if_edited();
    assert_eq!(s.material, "Aluminium");
    s.yield_strength = "300".into();
    s.custom_if_edited();
    assert_eq!(s.material, "Custom");
}

#[test]
fn safety_factor() {
    let r = Rebuilder::default().rebuild(&boxed());
    let run = |yield_strength: &str| {
        let (d, r) = with_study(Study { yield_strength: yield_strength.into(), ..tension(&r) });
        d.run_study(&r, 0)
    };
    let rep = run("250").unwrap();
    assert_eq!(rep.yield_strength, Some(250.0));
    near(rep.safety_factor().unwrap(), 50.0, 1e-7);
    let rep = run("").unwrap();
    assert_eq!((rep.yield_strength, rep.safety_factor()), (None, None));
    near(run("355").unwrap().safety_factor().unwrap(), 71.0, 1e-7);
    assert_eq!(run("0").map(drop), Err("yield strength must be positive".to_string()));
    let mut d = boxed();
    assert!(matches!(d.apply(Command::SetStudies(vec![Study { yield_strength: "2 +".into(), ..tension(&r) }])), Err(DocError::Param(_))));
}

#[test]
fn set_studies_checks_and_undoes() {
    let mut d = boxed();
    let mut h = History::default();
    let r = Rebuilder::default().rebuild(&d);
    h.execute(&mut d, vec![Command::SetStudies(vec![tension(&r)])]).unwrap();
    assert_eq!(d.studies().len(), 1);
    assert!(h.undo(&mut d));
    assert!(d.studies().is_empty());
    let bad = Study { young: "2 +".into(), ..tension(&r) };
    assert!(matches!(h.execute(&mut d, vec![Command::SetStudies(vec![bad])]), Err(DocError::Param(_))));
    assert!(d.studies().is_empty());
}

#[test]
fn studies_save_and_load() {
    let mut d = boxed();
    let r = Rebuilder::default().rebuild(&d);
    let bare = std::env::temp_dir().join(format!("cadtest_{}_study_bare.cadproj", std::process::id()));
    d.save(&bare).unwrap();
    let bare_text = std::fs::read_to_string(&bare).unwrap();
    std::fs::remove_file(&bare).ok();
    assert!(!bare_text.contains("\"studies\""));
    d.apply(Command::SetStudies(vec![tension(&r)])).unwrap();
    let p = std::env::temp_dir().join(format!("cadtest_{}_study.cadproj", std::process::id()));
    d.save(&p).unwrap();
    let text = std::fs::read_to_string(&p).unwrap();
    let d2 = Document::load(&p);
    std::fs::remove_file(&p).ok();
    assert_eq!(d2.unwrap().studies(), d.studies());
    assert!(text.contains("\"version\": 29") && text.contains("\"Force\"") && !text.contains("\"Pressure\""));
    assert!(text.contains("\"yield_strength\": \"250\""));
    let old = std::env::temp_dir().join(format!("cadtest_{}_study_old.cadproj", std::process::id()));
    std::fs::write(&old, r#"{"version":25,"name":"x","entities":{},"next_id":1}"#).unwrap();
    let d3 = Document::load(&old);
    std::fs::remove_file(&old).ok();
    assert!(d3.unwrap().studies().is_empty());
}

fn near(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} vs {b}");
}

/// A box document with `s` as its only study, and its rebuild.
fn with_study(s: Study) -> (Document, RebuildResult) {
    let mut d = boxed();
    d.apply(Command::SetStudies(vec![s])).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    (d, r)
}

fn tension_doc() -> (Document, RebuildResult) {
    let r = Rebuilder::default().rebuild(&boxed());
    with_study(tension(&r))
}

#[test]
fn tension_is_exact() {
    let (d, r) = tension_doc();
    let rep = d.run_study(&r, 0).unwrap();
    assert_eq!((rep.stats.nodes, rep.stats.elements, rep.result.dofs, rep.h), (765, 384, 2160, 5.0));
    for (p, u) in rep.mesh.nodes.iter().zip(&rep.result.displacement) {
        near(u.x, p.x / 42000.0, 1e-11);
        assert!(u.y.abs() <= 1e-11 && u.z.abs() <= 1e-11);
    }
    let (dmax, node) = rep.result.max_displacement();
    near(dmax, 0.0009523809523809524, 1e-11);
    near(rep.mesh.nodes[node].x, 40.0, 1e-9);
    near(rep.result.max_von_mises().0, 5.0, 1e-8);
    assert!(rep.result.applied.distance(DVec3::new(1000.0, 0.0, 0.0)) <= 1e-9);
    assert!(rep.result.reaction.distance(DVec3::new(-1000.0, 0.0, 0.0)) <= 1e-6);
    assert!(rep.result.residual <= 1e-9);
    assert_eq!(rep.accuracy.to_string(), "approximate (FEA, linear elastic)");
    assert_eq!(rep.label(), "approximate (FEA, linear elastic, mesh h=5 mm)");
    assert!(rep.mesh_ms >= 0.0 && rep.solve_ms >= 0.0);
}

#[test]
fn pressure_pushes_into_the_face() {
    let r = Rebuilder::default().rebuild(&boxed());
    let mut s = tension(&r);
    s.loads[0].kind = LoadKind::Pressure("5".into());
    let (d, r) = with_study(s);
    let rep = d.run_study(&r, 0).unwrap();
    assert!(rep.result.reaction.distance(DVec3::new(1000.0, 0.0, 0.0)) <= 1e-6);
    let (_, node) = rep.result.max_displacement();
    near(rep.result.displacement[node].x, -0.0009523809523809524, 1e-11);
}

#[test]
fn gravity_weighs_the_body() {
    let r = Rebuilder::default().rebuild(&boxed());
    let s = Study { gravity: true, loads: Vec::new(), fixed: vec![face(&r, -DVec3::X)], ..Study::new("Study 1".into(), 1, "5 mm".into()) };
    let (d, r) = with_study(s);
    let rep = d.run_study(&r, 0).unwrap();
    assert!(rep.result.applied.distance(DVec3::new(0.0, 0.0, -7.85 * 9.81e-6 * 8000.0)) <= 1e-12);
    assert!(rep.result.reaction.distance(DVec3::new(0.0, 0.0, 7.85 * 9.81e-6 * 8000.0)) <= 1e-9);
    assert!(rep.result.residual <= 1e-9);
    assert!(rep.result.max_displacement().0 > 0.0);
}

#[test]
fn run_refusals() {
    let r0 = Rebuilder::default().rebuild(&boxed());
    let run = |edit: &dyn Fn(&mut Study)| {
        let mut s = tension(&r0);
        edit(&mut s);
        let (d, r) = with_study(s);
        d.run_study(&r, 0).map(drop)
    };
    let (d, r) = tension_doc();
    assert_eq!(d.run_study(&r, 3).map(drop), Err("study 3 not found".to_string()));
    assert_eq!(run(&|s| s.young = "0".into()), Err("Young's modulus must be positive".to_string()));
    assert_eq!(run(&|s| s.poisson = "0.5".into()), Err("Poisson's ratio must be at least 0 and below 0.5".to_string()));
    assert_eq!(run(&|s| (s.gravity, s.density) = (true, "-1".into())), Err("density must not be negative".to_string()));
    assert_eq!(run(&|s| s.h = "0.1 mm".into()), Err("element size 0.1 mm gives 8000000 grid cells (at most 50000); use a larger element size".to_string()));
    assert_eq!(run(&|s| s.fixed.clear()), Err("fix at least one face".to_string()));
    assert_eq!(run(&|s| s.loads.clear()), Err("no load: add a force, a pressure or gravity".to_string()));
    let missing = FaceRef { key: FaceKey::Plane(DVec3::new(1.0, 1.0, 0.0).normalize()), at: DVec3::ZERO };
    assert_eq!(run(&|s| s.loads[0].face = missing), Err("face not found; pick it again".to_string()));
    // "nonsense" is not a parameter, so SetStudies accepts the text (it parses) but the run fails.
    assert!(run(&|s| s.young = "nonsense".into()).is_err());
}
