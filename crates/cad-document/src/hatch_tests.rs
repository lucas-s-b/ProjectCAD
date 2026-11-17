use crate::{Command, Document, Entity};
use cad_geometry::{DVec2, HatchPattern, Shape};

fn temp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("cadtest_{}_{name}.cadproj", std::process::id()))
}

#[test]
fn version_10_hatch_loads_as_lines_and_pattern_roundtrips() {
    let boundary = r#"[[0.0,0.0],[10.0,0.0],[10.0,10.0]]"#;
    let v10 = format!(r#"{{"version":10,"name":"x","next_id":2,"entities":{{"1":{{"id":1,"name":"h","shape":{{"Hatch":{{"boundary":{boundary},"angle":0.5,"spacing":2.0}}}}}}}}}}"#);
    let p = temp("hatch10");
    std::fs::write(&p, v10).unwrap();
    let d = Document::load(&p).unwrap();
    assert!(matches!(d.get(1).unwrap().shape, Shape::Hatch { pattern: HatchPattern::Lines, .. }));
    let mut d = Document::new("t");
    let b = vec![DVec2::ZERO, DVec2::new(10.0, 0.0), DVec2::new(10.0, 10.0)];
    let shape = Shape::Hatch { boundary: b, angle: 0.0, spacing: 2.0, pattern: HatchPattern::Solid };
    d.apply(Command::Add(Entity { id: 1, name: "h".into(), shape: shape.clone(), layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
    d.save(&p).unwrap();
    let l = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    assert_eq!(l.get(1).unwrap().shape, shape);
}
