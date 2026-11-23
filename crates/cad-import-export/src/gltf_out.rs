//! glTF 2.0 binary (.glb) export: each solid once as a mesh, placed by nodes; metres, Y up.
use cad_geometry::DAffine3;
use cad_modeler::Body;
use serde_json::json;

/// Solids written once and placed one or more times: what glTF and 3MF share.
pub struct Scene<'a> {
    /// Name and solid (mm, its own coordinates) of each mesh.
    pub meshes: Vec<(String, &'a Body)>,
    /// Name, index into `meshes` and placement (mesh coordinates to world, mm) of each placed item.
    pub items: Vec<(String, usize, DAffine3)>,
}

/// glTF uses metres.
const MM_TO_M: f64 = 0.001;
const GLB_MAGIC: u32 = 0x4654_6C67; // "glTF"
const CHUNK_JSON: u32 = 0x4E4F_534A; // "JSON"
const CHUNK_BIN: u32 = 0x004E_4942; // "BIN\0"
/// The app's body grey.
const GREY: [f32; 4] = [0.72, 0.75, 0.80, 1.0];
/// Column-major: Z up (CAD) to Y up (glTF): x stays, z becomes y, y becomes -z.
const Z_UP_TO_Y_UP: [f64; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// A .glb file: node 0 turns Z up into Y up and holds one node per item (its placement as `matrix`, translation in
/// metres); one mesh per solid with one vertex per triangle corner (POSITION in metres, NORMAL), no indices; one grey
/// material.
pub fn to_glb(scene: &Scene) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let (mut views, mut accessors, mut meshes) = (Vec::new(), Vec::new(), Vec::new());
    for (name, body) in &scene.meshes {
        let m = &body.mesh;
        // One vertex per corner: the body normals are per corner (sharp between faces).
        let pos: Vec<[f32; 3]> = m.indices.iter().map(|&i| (m.positions[i as usize] * MM_TO_M).as_vec3().to_array()).collect();
        let nrm: Vec<[f32; 3]> = m.normals.iter().map(|n| n.as_vec3().to_array()).collect();
        let lo = pos.iter().fold([f32::INFINITY; 3], |a, p| [0, 1, 2].map(|k| a[k].min(p[k])));
        let hi = pos.iter().fold([f32::NEG_INFINITY; 3], |a, p| [0, 1, 2].map(|k| a[k].max(p[k])));
        let first = accessors.len();
        for (data, bounds) in [(&pos, Some((lo, hi))), (&nrm, None)] {
            views.push(json!({ "buffer": 0, "byteOffset": bin.len(), "byteLength": data.len() * 12, "target": 34962 }));
            bin.extend(data.iter().flatten().flat_map(|v| v.to_le_bytes()));
            let mut acc = json!({ "bufferView": views.len() - 1, "componentType": 5126, "count": data.len(), "type": "VEC3" });
            if let Some((lo, hi)) = bounds {
                acc["min"] = json!(lo);
                acc["max"] = json!(hi);
            }
            accessors.push(acc);
        }
        meshes.push(json!({ "name": name, "primitives": [{ "attributes": { "POSITION": first, "NORMAL": first + 1 }, "material": 0, "mode": 4 }] }));
    }
    let mut nodes = vec![json!({ "name": "ProjectCAD (Z up)", "matrix": Z_UP_TO_Y_UP, "children": (1..=scene.items.len()).collect::<Vec<_>>() })];
    for (name, mesh, at) in &scene.items {
        let m = at.matrix3;
        // Column-major; rotation stays, only the translation is a length.
        let mut matrix = [0.0; 16];
        for (k, v) in [m.x_axis, m.y_axis, m.z_axis, at.translation * MM_TO_M].iter().enumerate() {
            matrix[4 * k..4 * k + 3].copy_from_slice(&v.to_array());
        }
        matrix[15] = 1.0;
        nodes.push(json!({ "name": name, "mesh": mesh, "matrix": matrix }));
    }
    let doc = json!({
        "asset": { "version": "2.0", "generator": "ProjectCAD" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": nodes,
        "meshes": meshes,
        "materials": [{ "name": "ProjectCAD grey", "pbrMetallicRoughness": { "baseColorFactor": GREY, "metallicFactor": 0.0, "roughnessFactor": 0.6 } }],
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{ "byteLength": bin.len() }],
    });
    let mut text = serde_json::to_vec(&doc).expect("numbers and strings only");
    // Chunks are 4-byte aligned: JSON padded with spaces, binary with zeros.
    text.resize(text.len().next_multiple_of(4), b' ');
    bin.resize(bin.len().next_multiple_of(4), 0);
    let total = 12 + 8 + text.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    for v in [GLB_MAGIC, 2, total as u32, text.len() as u32, CHUNK_JSON] {
        out.extend(v.to_le_bytes());
    }
    out.extend(&text);
    for v in [bin.len() as u32, CHUNK_BIN] {
        out.extend(v.to_le_bytes());
    }
    out.extend(&bin);
    out
}

#[cfg(test)]
#[path = "gltf_out_tests.rs"]
mod tests;
