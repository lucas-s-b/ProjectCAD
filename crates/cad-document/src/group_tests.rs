use crate::{Command, DocError, Document, Entity, EntityId, History};
use cad_geometry::{DVec2, Shape};

fn add(id: EntityId) -> Command {
    let shape = Shape::Circle { center: DVec2::new(id as f64, 0.0), radius: 1.0 };
    Command::Add(Entity { id, name: format!("e{id}"), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })
}

fn three() -> (Document, History) {
    let mut d = Document::new("t");
    let mut h = History::default();
    h.execute(&mut d, vec![add(1), add(2), add(3)]).unwrap();
    (d, h)
}

fn groups(d: &Document) -> Vec<Option<u32>> {
    d.entities().map(|e| e.group).collect()
}

fn temp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("cadtest_{}_{name}.cadproj", std::process::id()))
}

#[test]
fn set_group_inverse_restores_each_previous_group() {
    let (mut d, mut h) = three();
    h.execute(&mut d, vec![Command::SetGroup { ids: vec![1, 2], group: Some(4) }]).unwrap();
    h.execute(&mut d, vec![Command::SetGroup { ids: vec![2, 3], group: Some(9) }]).unwrap();
    assert_eq!(groups(&d), [Some(4), Some(9), Some(9)]);
    assert_eq!(d.group_members(9), [2, 3]);
    assert!(h.undo(&mut d));
    assert_eq!(groups(&d), [Some(4), Some(4), None]);
    assert!(h.undo(&mut d));
    assert_eq!(groups(&d), [None, None, None]);
    assert!(h.redo(&mut d) && h.redo(&mut d));
    assert_eq!(groups(&d), [Some(4), Some(9), Some(9)]);
    h.execute(&mut d, vec![Command::SetGroup { ids: vec![1, 2, 3], group: None }]).unwrap();
    assert_eq!(groups(&d), [None, None, None]);
    assert!(h.undo(&mut d));
    assert_eq!(groups(&d), [Some(4), Some(9), Some(9)]);
}

#[test]
fn set_group_needs_every_entity_and_changes_nothing_on_error() {
    let (mut d, _) = three();
    let rev = d.revision();
    let r = d.apply(Command::SetGroup { ids: vec![1, 7], group: Some(1) });
    assert!(matches!(r, Err(DocError::NotFound(7))));
    assert_eq!((groups(&d), d.revision()), (vec![None; 3], rev));
}

#[test]
fn meta_set_remove_and_inverse() {
    let (mut d, mut h) = three();
    let set = |value: Option<&str>| Command::SetMeta { id: 2, key: "material".into(), value: value.map(String::from) };
    h.execute(&mut d, vec![set(Some("steel"))]).unwrap();
    h.execute(&mut d, vec![set(Some("brass"))]).unwrap();
    assert_eq!(d.get(2).unwrap().meta["material"], "brass");
    h.execute(&mut d, vec![set(None)]).unwrap();
    assert!(d.get(2).unwrap().meta.is_empty());
    assert!(h.undo(&mut d));
    assert_eq!(d.get(2).unwrap().meta["material"], "brass");
    assert!(h.undo(&mut d));
    assert_eq!(d.get(2).unwrap().meta["material"], "steel");
    assert!(h.undo(&mut d));
    assert!(d.get(2).unwrap().meta.is_empty());
    let empty = Command::SetMeta { id: 2, key: " ".into(), value: Some("x".into()) };
    assert!(matches!(d.apply(empty), Err(DocError::BadFeature("empty key"))));
    let missing = Command::SetMeta { id: 9, key: "k".into(), value: None };
    assert!(matches!(d.apply(missing), Err(DocError::NotFound(9))));
}

#[test]
fn group_ids_are_allocated_after_load_and_data_roundtrips() {
    let (mut d, mut h) = three();
    assert_eq!(d.alloc_group_id(), 1);
    h.execute(&mut d, vec![Command::SetGroup { ids: vec![1, 3], group: Some(6) }]).unwrap();
    h.execute(&mut d, vec![Command::SetMeta { id: 3, key: "k".into(), value: Some("v".into()) }]).unwrap();
    let p = temp("group");
    d.save(&p).unwrap();
    let mut l = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    assert_eq!(groups(&l), [Some(6), None, Some(6)]);
    assert_eq!(l.get(3).unwrap().meta["k"], "v");
    assert_eq!(l.alloc_group_id(), 7);
    // A stale counter in the file does not hand out a used id.
    let stale = r#"{"version":11,"name":"x","next_id":2,"next_group_id":1,"entities":{"1":{"id":1,"name":"c","group":5,
        "shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}}}}"#;
    std::fs::write(&p, stale).unwrap();
    let mut s = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    assert_eq!(s.alloc_group_id(), 6);
}

#[test]
fn version_10_file_loads_without_groups() {
    let v10 = r#"{"version":10,"name":"x","next_id":2,"entities":{"1":{"id":1,"name":"l","shape":{"Line":{"a":[0.0,0.0],"b":[1.0,0.0]}}}}}"#;
    let p = temp("v10group");
    std::fs::write(&p, v10).unwrap();
    let mut d = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    let e = d.get(1).unwrap();
    assert!(e.group.is_none() && e.meta.is_empty());
    assert_eq!(d.alloc_group_id(), 1);
    assert_eq!(d.dim_links().count(), 0);
}
