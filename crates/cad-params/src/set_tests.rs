use super::*;
use crate::value::Dim;

fn mm(v: f64) -> Value {
    Value { v, dim: Dim::LENGTH }
}

fn set_of(items: &[(&str, &str)]) -> ParamSet {
    let mut s = ParamSet::default();
    for (n, e) in items {
        s.set(n, e).unwrap();
    }
    s
}

fn names(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn brief_example() {
    let s = set_of(&[("width", "100 mm"), ("height", "width * 0.5"), ("radius", "height / 4")]);
    let r = s.evaluate();
    assert_eq!(r["width"], Ok(mm(100.0)));
    assert_eq!(r["height"], Ok(mm(50.0)));
    assert_eq!(r["radius"], Ok(mm(12.5)));
}

#[test]
fn order_independent_of_insertion() {
    let s = set_of(&[("radius", "height / 4"), ("height", "width * 0.5"), ("width", "100 mm")]);
    assert_eq!(s.order().unwrap(), strs(&["width", "height", "radius"]));
    assert_eq!(s.evaluate()["radius"], Ok(mm(12.5)));
    // Independent entries keep display order.
    let t = set_of(&[("b", "1"), ("a", "2"), ("c", "a + b")]);
    assert_eq!(t.order().unwrap(), strs(&["b", "a", "c"]));
}

#[test]
fn dependencies_and_dependents() {
    let s = set_of(&[("width", "100 mm"), ("height", "width * 0.5"), ("radius", "height / 4"), ("other", "3")]);
    assert_eq!(s.dependencies("height"), names(&["width"]));
    assert_eq!(s.dependents("width"), names(&["height", "radius"]));
    assert!(s.dependents("other").is_empty());
    assert!(s.dependencies("nope").is_empty());
}

#[test]
fn two_cycle_and_self_reference() {
    let s = set_of(&[("a", "b + 1"), ("b", "a + 1"), ("s", "s * 2"), ("user", "a + 1"), ("free", "5")]);
    let r = s.evaluate();
    assert_eq!(r["a"], Err(ParamError::Cycle(strs(&["a", "b", "a"]))));
    assert_eq!(r["b"], Err(ParamError::Cycle(strs(&["b", "a", "b"]))));
    assert_eq!(r["s"], Err(ParamError::Cycle(strs(&["s", "s"]))));
    assert_eq!(r["user"], Err(ParamError::Depends("a".into())));
    assert_eq!(r["free"].as_ref().unwrap().v, 5.0);
    assert!(matches!(s.order(), Err(ParamError::Cycle(c)) if c.first() == c.last()));
}

#[test]
fn downstream_of_a_cycle_chain() {
    let s = set_of(&[("a", "b"), ("b", "a"), ("c", "a"), ("d", "c")]);
    let r = s.evaluate();
    assert_eq!(r["c"], Err(ParamError::Depends("a".into())));
    assert_eq!(r["d"], Err(ParamError::Depends("c".into())));
}

#[test]
fn missing_name_and_depends() {
    let mut s = set_of(&[("a", "missing + 1"), ("b", "a * 2")]);
    let r = s.evaluate();
    assert_eq!(r["a"], Err(ParamError::UnknownName("missing".into())));
    assert_eq!(r["b"], Err(ParamError::Depends("a".into())));
    s.set("missing", "4").unwrap();
    assert_eq!(s.evaluate()["b"].as_ref().unwrap().v, 10.0);
}

#[test]
fn replacing_changes_dependents() {
    let mut s = set_of(&[("width", "100 mm"), ("height", "width * 0.5")]);
    assert_eq!(s.evaluate()["height"], Ok(mm(50.0)));
    s.set("width", "200 mm").unwrap();
    assert_eq!(s.evaluate()["height"], Ok(mm(100.0)));
    assert_eq!(s.iter().map(|(n, _)| n).collect::<Vec<_>>(), ["width", "height"]);
    assert_eq!(s.len(), 2);
}

#[test]
fn eval_error_propagates() {
    let s = set_of(&[("a", "1 / 0"), ("b", "a + 1")]);
    let r = s.evaluate();
    assert_eq!(r["a"], Err(ParamError::Domain("division by zero")));
    assert_eq!(r["b"], Err(ParamError::Depends("a".into())));
}

#[test]
fn rename_rewrites_whole_identifiers() {
    let mut s = set_of(&[("width", "100 mm"), ("width2", "5 mm"), ("height", "width * 0.5 + width2")]);
    s.rename("width", "w").unwrap();
    assert_eq!(s.get("height"), Some("w * 0.5 + width2"));
    assert_eq!(s.get("w"), Some("100 mm"));
    assert_eq!(s.get("width"), None);
    assert_eq!(s.evaluate()["height"], Ok(mm(55.0)));
    assert_eq!(s.rename("w", "width2"), Err(ParamError::InvalidName("width2".into())));
    assert_eq!(s.rename("w", "mm"), Err(ParamError::InvalidName("mm".into())));
    assert_eq!(s.rename("zzz", "q"), Err(ParamError::UnknownName("zzz".into())));
}

#[test]
fn invalid_names() {
    let mut s = ParamSet::default();
    for bad in ["mm", "sin", "pi", "e", "2x", "", "a b", "a-b", "if"] {
        assert_eq!(s.set(bad, "1"), Err(ParamError::InvalidName(bad.to_string())), "{bad:?}");
    }
    assert!(s.is_empty());
    assert!(matches!(s.set("ok", "1 +"), Err(ParamError::Syntax { .. })));
    assert!(s.is_empty());
}

#[test]
fn remove_entry() {
    let mut s = set_of(&[("a", "1"), ("b", "a")]);
    assert!(s.remove("a"));
    assert!(!s.remove("a"));
    assert_eq!(s.evaluate()["b"], Err(ParamError::UnknownName("a".into())));
}

#[test]
fn free_expression() {
    let s = set_of(&[("width", "100 mm"), ("height", "width * 0.5"), ("bad", "1 / 0")]);
    assert_eq!(s.eval_expr("height * 2"), Ok(mm(100.0)));
    assert_eq!(s.eval_expr("bad + 1"), Err(ParamError::Depends("bad".into())));
    assert_eq!(s.eval_expr("nope"), Err(ParamError::UnknownName("nope".into())));
    assert!(matches!(s.eval_expr("1 +"), Err(ParamError::Syntax { .. })));
}

#[test]
fn serde_round_trip() {
    let s = set_of(&[("width", "100 mm"), ("height", "width * 0.5")]);
    let json = serde_json::to_string(&s).unwrap();
    let back: ParamSet = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.evaluate()["height"], Ok(mm(50.0)));
}

#[test]
fn deserialized_invalid_entries_keep_their_error() {
    let json = r#"{"entries":[["2x","1"],["a","1 +"],["b","a"],["ok","3"]]}"#;
    let s: ParamSet = serde_json::from_str(json).unwrap();
    let r = s.evaluate();
    assert_eq!(r["2x"], Err(ParamError::InvalidName("2x".into())));
    assert!(matches!(r["a"], Err(ParamError::Syntax { .. })));
    assert_eq!(r["b"], Err(ParamError::Depends("a".into())));
    assert_eq!(r["ok"].as_ref().unwrap().v, 3.0);
    assert_eq!(s.len(), 4);
}

#[test]
fn long_chain() {
    let mut s = ParamSet::default();
    s.set("p0", "1").unwrap();
    for i in 1..1000 {
        s.set(&format!("p{i}"), &format!("p{} + 1", i - 1)).unwrap();
    }
    assert_eq!(s.evaluate()["p999"].as_ref().unwrap().v, 1000.0);
    assert_eq!(s.order().unwrap().len(), 1000);
    assert_eq!(s.dependents("p0").len(), 999);
}
