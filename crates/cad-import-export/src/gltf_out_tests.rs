use super::*;
use cad_geometry::{DVec2, DVec3, Shape, profile::Profile};
use cad_modeler::{Plane3, extrude};
use serde_json::Value;

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

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Checks the container and returns the JSON and the BIN chunk.
fn read_glb(bytes: &[u8]) -> (Value, Vec<u8>) {
    assert_eq!(u32_at(bytes, 0), 0x4654_6C67);
    assert_eq!(u32_at(bytes, 4), 2);
    assert_eq!(u32_at(bytes, 8) as usize, bytes.len());
    let (jl, jt) = (u32_at(bytes, 12) as usize, u32_at(bytes, 16));
    assert_eq!((jt, jl % 4), (0x4E4F_534A, 0));
    let json = serde_json::from_slice(&bytes[20..20 + jl]).unwrap();
    let at = 20 + jl;
    let (bl, bt) = (u32_at(bytes, at) as usize, u32_at(bytes, at + 4));
    assert_eq!((bt, bl % 4), (0x004E_4942, 0));
    assert_eq!(at + 8 + bl, bytes.len());
    (json, bytes[at + 8..].to_vec())
}

fn floats(bin: &[u8], doc: &Value, accessor: usize) -> Vec<[f32; 3]> {
    let acc = &doc["accessors"][accessor];
    let view = &doc["bufferViews"][acc["bufferView"].as_u64().unwrap() as usize];
    let start = view["byteOffset"].as_u64().unwrap() as usize;
    let data = &bin[start..start + view["byteLength"].as_u64().unwrap() as usize];
    data.as_chunks::<12>().0.iter().map(|c| [0, 1, 2].map(|k| f32::from_le_bytes([c[4 * k], c[4 * k + 1], c[4 * k + 2], c[4 * k + 3]]))).collect()
}

fn shift(x: f64) -> DAffine3 {
    DAffine3::from_translation(DVec3::new(x, 0.0, 0.0))
}

#[test]
fn glb_layout_and_lengths() {
    let b = cuboid();
    let scene = Scene { meshes: vec![("box".into(), &b)], items: vec![("a".into(), 0, DAffine3::IDENTITY), ("b".into(), 0, shift(100.0))] };
    let (doc, bin) = read_glb(&to_glb(&scene));
    let blen = doc["buffers"][0]["byteLength"].as_u64().unwrap() as usize;
    assert!(blen <= bin.len() && blen + 3 >= bin.len());
    for v in doc["bufferViews"].as_array().unwrap() {
        assert!(v["byteOffset"].as_u64().unwrap() + v["byteLength"].as_u64().unwrap() <= blen as u64);
    }
    for a in doc["accessors"].as_array().unwrap() {
        let view = &doc["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
        assert_eq!(a["count"].as_u64().unwrap() * 12, view["byteLength"].as_u64().unwrap());
    }
    let acc = &doc["accessors"][0];
    for (k, (lo, hi)) in [(0.0, 0.04), (0.0, 0.02), (0.0, 0.01)].into_iter().enumerate() {
        assert!((acc["min"][k].as_f64().unwrap() - lo).abs() < 1e-6);
        assert!((acc["max"][k].as_f64().unwrap() - hi).abs() < 1e-6);
    }
    let pos = floats(&bin, &doc, 0);
    let p = |q: [f32; 3]| DVec3::from(q.map(f64::from));
    let volume: f64 = pos.as_chunks::<3>().0.iter().map(|t| p(t[0]).dot(p(t[1]).cross(p(t[2]))) / 6.0).sum::<f64>() * 1e9;
    assert!((volume - b.volume()).abs() <= 1e-5 * b.volume(), "{volume}");
    assert!(floats(&bin, &doc, 1).iter().all(|n| (p(*n).length() - 1.0).abs() < 1e-5));
}

#[test]
fn glb_nodes_share_one_mesh() {
    let (a, c) = (cuboid(), cylinder());
    let scene = Scene {
        meshes: vec![("box".into(), &a), ("cyl".into(), &c)],
        items: vec![("a".into(), 0, DAffine3::IDENTITY), ("b".into(), 1, DAffine3::IDENTITY), ("c".into(), 0, shift(100.0))],
    };
    let (doc, _) = read_glb(&to_glb(&scene));
    assert_eq!(doc["meshes"].as_array().unwrap().len(), 2);
    assert_eq!(doc["accessors"].as_array().unwrap().len(), 4);
    assert_eq!(doc["nodes"].as_array().unwrap().len(), 4);
    assert_eq!(doc["nodes"][0]["children"], serde_json::json!([1, 2, 3]));
    assert_eq!(doc["nodes"][3]["mesh"], 0);
    assert!((doc["nodes"][3]["matrix"][12].as_f64().unwrap() - 0.1).abs() < 1e-12);
    assert_eq!(doc["nodes"][0]["matrix"][6].as_f64(), Some(-1.0));
    assert_eq!(doc["nodes"][0]["matrix"][9].as_f64(), Some(1.0));
}

#[test]
fn glb_names_are_kept() {
    let b = cuboid();
    let scene = Scene { meshes: vec![("box".into(), &b)], items: vec![("Gear \"#1\"".into(), 0, DAffine3::IDENTITY)] };
    let (doc, _) = read_glb(&to_glb(&scene));
    assert_eq!(doc["nodes"][1]["name"], "Gear \"#1\"");
}
