use super::*;
use crate::Scene;
use crate::zip_out::tests::unzip;
use cad_geometry::{DAffine3, DVec2, DVec3, Shape, profile::Profile};
use cad_modeler::{Body, Plane3, extrude};

fn body(shape: Shape, z0: f64, height: f64) -> Body {
    let profile = Profile::from_shapes(&[&shape]).unwrap();
    let plane = Plane3 { origin: DVec3::new(0.0, 0.0, z0), ..Plane3::XY };
    Body::from_brep(&extrude(&profile, &plane, height).unwrap(), 0.01).unwrap()
}

fn cuboid() -> Body {
    body(Shape::rect(DVec2::ZERO, DVec2::new(40.0, 20.0)), 0.0, 10.0)
}

fn cylinder() -> Body {
    body(Shape::Circle { center: DVec2::new(5.0, 5.0), radius: 7.0 }, 0.0, 12.0)
}

fn scene<'a>(a: &'a Body, c: &'a Body) -> Scene<'a> {
    Scene {
        meshes: vec![("box <1>".into(), a), ("cyl".into(), c)],
        items: vec![
            ("a".into(), 0, DAffine3::IDENTITY),
            ("b".into(), 1, DAffine3::IDENTITY),
            ("c".into(), 0, DAffine3::from_translation(DVec3::new(100.0, 0.0, 0.0))),
        ],
    }
}

#[test]
fn three_mf_package_has_its_parts() {
    let (a, c) = (cuboid(), cylinder());
    let files = unzip(&to_3mf(&scene(&a, &c)).unwrap());
    assert_eq!(files.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["[Content_Types].xml", "_rels/.rels", "3D/3dmodel.model"]);
    for (_, data) in &files[..2] {
        roxmltree::Document::parse(std::str::from_utf8(data).unwrap()).unwrap();
    }
}

#[test]
fn three_mf_objects_items_and_volume() {
    let (a, c) = (cuboid(), cylinder());
    let files = unzip(&to_3mf(&scene(&a, &c)).unwrap());
    let text = String::from_utf8(files[2].1.clone()).unwrap();
    let doc = roxmltree::Document::parse(&text).unwrap();
    assert_eq!(doc.root_element().attribute("unit"), Some("millimeter"));
    let objects: Vec<_> = doc.descendants().filter(|n| n.has_tag_name("object")).collect();
    assert_eq!(objects.iter().map(|o| o.attribute("id").unwrap()).collect::<Vec<_>>(), ["1", "2"]);
    assert_eq!(objects[0].attribute("name"), Some("box <1>"));
    let count = |o: &roxmltree::Node, tag: &str| o.descendants().filter(|n| n.has_tag_name(tag)).count();
    for (o, b) in objects.iter().zip([&a, &c]) {
        assert_eq!(count(o, "vertex"), b.mesh.positions.len());
        assert_eq!(count(o, "triangle"), b.mesh.triangle_count());
    }
    let items: Vec<_> = doc.descendants().filter(|n| n.has_tag_name("item")).collect();
    assert_eq!(items.iter().map(|i| i.attribute("objectid").unwrap()).collect::<Vec<_>>(), ["1", "2", "1"]);
    assert_eq!(items[2].attribute("transform"), Some("1 0 0 0 1 0 0 0 1 100 0 0"));
    let num = |n: roxmltree::Node, k: &str| n.attribute(k).unwrap().parse::<f64>().unwrap();
    let p: Vec<DVec3> = objects[0].descendants().filter(|n| n.has_tag_name("vertex")).map(|n| DVec3::new(num(n, "x"), num(n, "y"), num(n, "z"))).collect();
    let volume: f64 = objects[0]
        .descendants()
        .filter(|n| n.has_tag_name("triangle"))
        .map(|n| {
            let i = |k| num(n, k) as usize;
            p[i("v1")].dot(p[i("v2")].cross(p[i("v3")])) / 6.0
        })
        .sum();
    assert!((volume - a.volume()).abs() <= 1e-5 * a.volume(), "{volume}");
}
