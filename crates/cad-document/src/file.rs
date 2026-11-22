use crate::document::{DocError, Document, FORMAT_VERSION};
use cad_geometry::{Shape, Tolerance};
use std::path::Path;

impl Document {
    pub fn save(&self, path: &Path) -> Result<(), DocError> {
        let bytes = serde_json::to_vec_pretty(self)?;
        // Write beside the target, then rename: a crash must not truncate the project.
        let tmp = path.with_extension("cadproj.tmp");
        std::fs::write(&tmp, bytes)?;
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, DocError> {
        let bytes = std::fs::read(path)?;
        let mut doc: Document = serde_json::from_slice(&bytes)?;
        if doc.version > FORMAT_VERSION {
            return Err(DocError::UnsupportedVersion(doc.version));
        }
        if doc.settings.precision > 8 {
            return Err(DocError::BadPrecision);
        }
        let tol = Tolerance::default();
        for (key, e) in &doc.entities {
            e.shape.validate(&tol)?;
            if *key != e.id {
                return Err(DocError::DuplicateId(*key));
            }
        }
        if !doc.layers.contains_key(&0) {
            return Err(DocError::LayerNotFound(0));
        }
        for (key, l) in &doc.layers {
            if *key != l.id {
                return Err(DocError::LayerExists(*key));
            }
        }
        for l in doc.layers.values() {
            l.check_weight()?;
        }
        if let Some(e) = doc.entities.values().find(|e| !doc.layers.contains_key(&e.layer)) {
            return Err(DocError::LayerNotFound(e.layer));
        }
        if !doc.sketches.contains_key(&0) {
            return Err(DocError::SketchNotFound(0));
        }
        if let Some((key, _)) = doc.sketches.iter().find(|(key, s)| **key != s.id) {
            return Err(DocError::SketchExists(*key));
        }
        if let Some(e) = doc.entities.values().find(|e| !doc.sketches.contains_key(&e.sketch)) {
            return Err(DocError::SketchNotFound(e.sketch));
        }
        if let Some(e) = doc.entities.values().find(|e| e.shape.needs_level() && !doc.is_level(e.sketch)) {
            return Err(DocError::NotALevel(e.sketch));
        }
        for x in doc.entities.values() {
            if let Shape::Opening(o) = &x.shape {
                doc.check_host(x.sketch, o)?;
            }
        }
        if let Some(max) = doc.sketches.keys().next_back() {
            doc.next_sketch_id = doc.next_sketch_id.max(max.saturating_add(1));
        }
        if let Some(missing) = doc.constraints.values().flat_map(|c| c.entities()).find(|e| !doc.entities.contains_key(e)) {
            return Err(DocError::NotFound(missing));
        }
        if let Some(stray) = doc.constraint_exprs.keys().find(|id| !doc.constraints.contains_key(id)) {
            return Err(DocError::ConstraintNotFound(*stray));
        }
        if let Some(missing) = doc.features.iter().flat_map(|f| f.entities()).find(|e| !doc.entities.contains_key(e)) {
            return Err(DocError::NotFound(missing));
        }
        if let Some(missing) = doc.features.iter().filter_map(|f| f.source()).find(|s| doc.feature(*s).is_none()) {
            return Err(DocError::FeatureNotFound(missing));
        }
        for (dim, (p, q)) in &doc.dim_links {
            if let Some(missing) = [dim, &p.entity, &q.entity].into_iter().find(|e| !doc.entities.contains_key(e)) {
                return Err(DocError::NotFound(*missing));
            }
        }
        if let Some(max) = doc.entities.values().filter_map(|e| e.group).max() {
            doc.next_group_id = doc.next_group_id.max(max.saturating_add(1));
        }
        doc.next_group_id = doc.next_group_id.max(1);
        if let Some(max) = doc.features.iter().map(|f| f.id).max() {
            doc.next_feature_id = doc.next_feature_id.max(max.saturating_add(1));
        }
        doc.next_feature_id = doc.next_feature_id.max(1);
        if let Some(max) = doc.constraints.keys().next_back() {
            doc.next_constraint_id = doc.next_constraint_id.max(max.saturating_add(1));
        }
        doc.next_constraint_id = doc.next_constraint_id.max(1);
        if let Some(max) = doc.layers.keys().next_back() {
            doc.next_layer_id = doc.next_layer_id.max(max.saturating_add(1));
        }
        if let Some(max) = doc.entities.keys().next_back() {
            doc.next_id = doc.next_id.max(max.saturating_add(1));
        }
        doc.next_id = doc.next_id.max(1);
        doc.revision = 0;
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Command, Entity, Layer};
    use crate::feature::{BodyOp, Feature, FeatureKind, RevolveAxis, Target};
    use cad_geometry::{DVec2, DVec3, Shape};

    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("cadtest_{}_{name}.cadproj", std::process::id()))
    }

    #[test]
    fn save_then_load_roundtrip() {
        let mut d = Document::new("proj");
        d.apply(Command::AddLayer(Layer::new(1, "Walls"))).unwrap();
        for (i, shape) in [
            Shape::Line { a: DVec2::ZERO, b: DVec2::new(1.0, 2.0) },
            Shape::Sphere { center: DVec3::new(1.0, 2.0, 3.0), radius: 4.0 },
        ]
        .into_iter()
        .enumerate()
        {
            let id = d.alloc_id();
            d.apply(Command::Add(Entity { id, name: format!("e{i}"), shape, layer: 1, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
        }
        let p = temp("roundtrip");
        d.save(&p).unwrap();
        let mut l = Document::load(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(l.name, d.name);
        assert_eq!(l.entities().cloned().collect::<Vec<_>>(), d.entities().cloned().collect::<Vec<_>>());
        assert_eq!(l.alloc_id(), 3);
        assert_eq!(l.layers().cloned().collect::<Vec<_>>(), d.layers().cloned().collect::<Vec<_>>());
        assert_eq!(l.alloc_layer_id(), 2);
        assert!(!p.with_extension("cadproj.tmp").exists());
    }

    fn load_text(name: &str, text: &str) -> Result<Document, DocError> {
        let p = temp(name);
        std::fs::write(&p, text).unwrap();
        let r = Document::load(&p);
        std::fs::remove_file(&p).unwrap();
        r
    }

    #[test]
    fn load_rejects_bad_files() {
        let newer = r#"{"version":999,"name":"x","entities":{},"next_id":1}"#;
        assert!(matches!(load_text("ver", newer), Err(DocError::UnsupportedVersion(999))));
        assert!(matches!(load_text("junk", "{not json"), Err(DocError::Format(_))));
        let flat = r#"{"version":1,"name":"x","next_id":2,"entities":{"1":{"id":1,"name":"c",
            "shape":{"Circle":{"center":[0.0,0.0],"radius":0.0}}}}}"#;
        assert!(matches!(load_text("flat", flat), Err(DocError::Geometry(_))));
        let mismatch = r#"{"version":1,"name":"x","next_id":2,"entities":{"5":{"id":1,"name":"c",
            "shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}}}}"#;
        assert!(matches!(load_text("key", mismatch), Err(DocError::DuplicateId(5))));
    }

    #[test]
    fn version_1_file_loads_on_layer_0() {
        let text = r#"{"version":1,"name":"x","next_id":3,"entities":{
            "1":{"id":1,"name":"a","shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}},
            "2":{"id":2,"name":"b","shape":{"Circle":{"center":[1.0,0.0],"radius":1.0}}}}}"#;
        let mut d = load_text("v1", text).unwrap();
        assert_eq!(d.layers().map(|l| l.id).collect::<Vec<_>>(), [0]);
        assert!(d.entities().all(|e| e.layer == 0));
        assert_eq!(d.alloc_layer_id(), 1);
    }

    #[test]
    fn version_2_file_gets_default_settings_and_settings_roundtrip() {
        let text = r#"{"version":2,"name":"x","next_id":1,"entities":{}}"#;
        let d = load_text("v2", text).unwrap();
        assert_eq!(*d.settings(), crate::Settings::default());
        let mut d = Document::new("s");
        let s = crate::Settings { units: crate::Units::Foot, precision: 5 };
        d.apply(Command::SetSettings(s)).unwrap();
        let p = temp("settings");
        d.save(&p).unwrap();
        let l = Document::load(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(*l.settings(), s);
        let bad = r#"{"version":3,"name":"x","next_id":1,"entities":{},"settings":{"units":"Inch","precision":9}}"#;
        assert!(matches!(load_text("prec", bad), Err(DocError::BadPrecision)));
    }

    #[test]
    fn load_rejects_bad_layers() {
        let missing = r#"{"version":2,"name":"x","next_id":2,"entities":{"1":{"id":1,"name":"c","layer":4,
            "shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}}}}"#;
        assert!(matches!(load_text("nolayer", missing), Err(DocError::LayerNotFound(4))));
        let no0 = r#"{"version":2,"name":"x","next_id":1,"entities":{},
            "layers":{"1":{"id":1,"name":"a","color":[0,0,0],"visible":true,"locked":false}}}"#;
        assert!(matches!(load_text("no0", no0), Err(DocError::LayerNotFound(0))));
        let key = r#"{"version":2,"name":"x","next_id":1,"entities":{},"layers":{
            "0":{"id":0,"name":"0","color":[0,0,0],"visible":true,"locked":false},
            "3":{"id":1,"name":"a","color":[0,0,0],"visible":true,"locked":false}}}"#;
        assert!(matches!(load_text("key3", key), Err(DocError::LayerExists(3))));
    }

    #[test]
    fn alloc_id_after_load_does_not_collide() {
        // next_id is stale (1) while entity 7 exists.
        let text = r#"{"version":1,"name":"x","next_id":1,"entities":{"7":{"id":7,"name":"c",
            "shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}}}}"#;
        let mut d = load_text("alloc", text).unwrap();
        assert_eq!(d.alloc_id(), 8);
    }

    const FEATURE: &str = r#"{"id":4,"name":"e","suppressed":false,
        "kind":{"Extrude":{"profile":[1],"distance":"t","offset":"0","op":"Add"}}}"#;

    #[test]
    fn params_and_features_roundtrip() {
        let mut d = Document::new("f");
        let id = d.alloc_id();
        d.apply(Command::Add(Entity { id, name: "r".into(), shape: Shape::rect(DVec2::ZERO, DVec2::ONE), layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
        d.apply(Command::SetParam { name: "t".into(), expr: Some("5 mm".into()) }).unwrap();
        let kind = FeatureKind::Revolve { profile: vec![id], axis: RevolveAxis::Line(id), angle: "90 deg".into(), op: BodyOp::Cut, target: Target::Auto };
        let feature = Feature { id: d.alloc_feature_id(), name: "rev".into(), suppressed: true, kind };
        d.apply(Command::AddFeature { index: 0, feature: feature.clone() }).unwrap();
        let p = temp("features");
        d.save(&p).unwrap();
        let mut l = Document::load(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(l.features(), [feature]);
        assert_eq!(l.params(), d.params());
        assert_eq!(l.alloc_feature_id(), 2);
    }

    #[test]
    fn version_5_file_loads_without_features() {
        let text = r#"{"version":5,"name":"x","next_id":1,"entities":{},"next_constraint_id":1}"#;
        let mut d = load_text("v5", text).unwrap();
        assert!(d.features().is_empty() && d.params().is_empty());
        assert_eq!(d.alloc_feature_id(), 1);
    }

    #[test]
    fn load_checks_features() {
        let shape = r#""shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}"#;
        let text = |entities: &str| {
            format!(r#"{{"version":6,"name":"x","next_id":2,"next_feature_id":1,"entities":{{{entities}}},"features":[{FEATURE}]}}"#)
        };
        assert!(matches!(load_text("nofe", &text("")), Err(DocError::NotFound(1))));
        let mut d = load_text("fe", &text(&format!(r#""1":{{"id":1,"name":"c",{shape}}}"#))).unwrap();
        assert_eq!(d.alloc_feature_id(), 5);
    }


    #[test]
    fn version_7_file_loads_and_takes_blend_features() {
        let entity = r#""1":{"id":1,"name":"c","shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}}"#;
        let fillet = r#"{"id":5,"name":"b","suppressed":false,"kind":{"Fillet":{"edges":[[0.0,0.0,5.0]],"radius":"3 mm"}}}"#;
        let text = format!(r#"{{"version":7,"name":"x","next_id":2,"next_feature_id":1,"entities":{{{entity}}},"features":[{FEATURE},{fillet}]}}"#);
        let mut d = load_text("v7", &text).unwrap();
        assert_eq!(d.features().len(), 2);
        assert!(matches!(&d.features()[1].kind, FeatureKind::Fillet { edges, faces, radius } if edges == &[DVec3::new(0.0, 0.0, 5.0)] && faces.is_empty() && radius == "3 mm"));
        assert_eq!(d.alloc_feature_id(), 6);
    }

    #[test]
    fn version_4_layer_gets_line_defaults_and_weight_is_checked() {
        let v4 = r#"{"version":4,"name":"x","next_id":1,"entities":{},"layers":{
            "0":{"id":0,"name":"0","color":[0,0,0],"visible":true,"locked":false}}}"#;
        let d = load_text("v4", v4).unwrap();
        let l = d.layer(0).unwrap();
        assert_eq!((l.line_type, l.line_weight), (crate::LineType::Continuous, 0.25));
        let bad = v4.replace(r#""locked":false"#, r#""locked":false,"line_weight":5.0"#);
        assert!(matches!(load_text("w", &bad), Err(DocError::BadLineWeight)));
    }
}
