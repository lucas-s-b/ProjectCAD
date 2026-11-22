use super::*;
use crate::rebuild_pattern::tests::{extrude, push, put, rect};
use crate::*;

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs().max(1.0), "{a} vs {b}");
}
fn part(id: FeatureId, def: PartDef, x: &str) -> Feature {
    let kind = FeatureKind::Part { def, sketch: 0, x: x.into(), y: "0".into(), op: BodyOp::New, target: Target::Auto };
    Feature { id, name: format!("part{id}"), suppressed: false, kind }
}
fn comp(id: ComponentId, body: FeatureId, material: &str, part_number: &str) -> Component {
    let mut meta = std::collections::BTreeMap::new();
    for (k, v) in [("material", material), ("part_number", part_number)] {
        if !v.is_empty() {
            meta.insert(k.to_string(), v.to_string());
        }
    }
    Component { id, name: format!("c{id}"), body, at: ["0".into(), "0".into(), "0".into()], angles: ["0".into(), "0".into(), "0".into()], meta, grounded: false }
}
/// Volume of the default shaft at BODY_TOLERANCE (50 facets), exact for the turned mesh.
const SHAFT: f64 = 19372.340134589296;

fn document() -> Document {
    let mut d = Document::new("bom");
    push(&mut d, part(1, PartDef::defaults()[1].clone(), "0"));
    push(&mut d, part(2, PartDef::defaults()[2].clone(), "100 mm"));
    push(&mut d, part(3, PartDef::defaults()[1].clone(), "200 mm"));
    put(&mut d, 1, 0, rect(300.0, 0.0, 310.0, 20.0));
    put(&mut d, 2, 0, rect(320.0, 0.0, 330.0, 20.0));
    push(&mut d, extrude(4, 1, "5 mm", BodyOp::New));
    push(&mut d, extrude(5, 2, "5 mm", BodyOp::New));
    for c in [
        comp(1, 1, "Steel", ""),
        comp(2, 1, "Steel", ""),
        comp(3, 2, "", ""),
        comp(4, 3, "Steel", ""),
        comp(5, 4, "steel", ""),
        comp(6, 5, "", ""),
        comp(7, 4, "steel", ""),
    ] {
        d.apply(Command::AddComponent(c)).unwrap();
    }
    d
}

fn rows_of(d: &Document) -> Vec<BomRow> {
    d.bom(&Rebuilder::default().rebuild(d))
}

fn texts(r: &BomRow) -> (usize, &str, &str, &str, &str, usize) {
    (r.item, &r.name, &r.designation, &r.material, &r.part_number, r.quantity)
}

#[test]
fn bom_groups_by_designation_material_and_part_number() {
    let mut d = document();
    let rows = rows_of(&d);
    assert_eq!(rows.len(), 4);
    let shaft = "Shaft \u{d8}20x80";
    assert_eq!(texts(&rows[0]), (1, "part1", shaft, "Steel", "", 3));
    close(rows[0].mass.unwrap(), SHAFT * 7.85 / 1000.0, 1e-9);
    assert_eq!(texts(&rows[1]), (2, "part2", "Compression spring d2 D20 L32", "", "", 1));
    assert_eq!(rows[1].mass, None);
    assert_eq!(texts(&rows[2]), (3, "f4", "", "steel", "", 2));
    close(rows[2].mass.unwrap(), 7.85, 1e-9);
    assert_eq!(texts(&rows[3]), (4, "f5", "", "", "", 1));
    assert_eq!(rows[3].mass, None);

    d.apply(Command::SetComponent(comp(4, 3, "Aluminium", "P-7"))).unwrap();
    let rows = rows_of(&d);
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0].quantity, 2);
    assert_eq!(texts(&rows[2]), (3, "part3", shaft, "Aluminium", "P-7", 1));
    close(rows[2].mass.unwrap(), SHAFT * 2.7 / 1000.0, 1e-9);
    assert_eq!((rows[3].name.as_str(), rows[3].quantity), ("f4", 2));
    assert_eq!((rows[4].name.as_str(), rows[4].quantity), ("f5", 1));

    let empty = Document::new("x");
    assert!(empty.bom(&Rebuilder::default().rebuild(&empty)).is_empty());
}

#[test]
fn bom_csv_of_a_document() {
    let rows = rows_of(&document());
    let csv = bom_csv(&rows);
    assert!(csv.starts_with(
        "\u{feff}Item,Part,Designation,Material,Part number,Quantity,Mass (g)\n1,part1,Shaft \u{d8}20x80,Steel,,3,152.1\n2,part2,Compression spring d2 D20 L32,,,1,\n"
    ));
    assert!(csv.ends_with("4,f5,,,,1,\n"));
    assert_eq!(csv.lines().count(), 5);
}

#[test]
fn bom_csv_quotes_fields() {
    let rows = [
        BomRow {
            item: 1,
            name: "Gear, big".into(),
            designation: "Spur gear m2 z20 b10".into(),
            material: "Steel".into(),
            part_number: String::new(),
            quantity: 2,
            mass: Some(92.3003),
        },
        BomRow {
            item: 2,
            name: "Pin \"A\"".into(),
            designation: String::new(),
            material: String::new(),
            part_number: "P-1".into(),
            quantity: 1,
            mass: None,
        },
    ];
    assert_eq!(
        bom_csv(&rows),
        "\u{feff}Item,Part,Designation,Material,Part number,Quantity,Mass (g)\n1,\"Gear, big\",Spur gear m2 z20 b10,Steel,,2,92.3\n2,\"Pin \"\"A\"\"\",,,P-1,1,\n"
    );
    assert_eq!(bom_csv(&[]), "\u{feff}Item,Part,Designation,Material,Part number,Quantity,Mass (g)\n");
}
