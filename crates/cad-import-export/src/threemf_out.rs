//! 3MF (3D Manufacturing Format, core specification) export: each solid once as an object, placed by build items; mm.
use crate::svg::escape;
use crate::zip_out::zip_stored;
use std::fmt::Write;

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/></Types>"#;
const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/></Relationships>"#;

/// A .3mf package: `[Content_Types].xml`, `_rels/.rels` and `3D/3dmodel.model` (unit millimeter) with one object per
/// mesh of `scene` (ids from 1) and one build item per item, its placement as `transform`.
pub fn to_3mf(scene: &crate::Scene) -> Result<Vec<u8>, String> {
    let mut m = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n<metadata name=\"Application\">ProjectCAD</metadata>\n<resources>\n",
    );
    for (k, (name, body)) in scene.meshes.iter().enumerate() {
        let _ = write!(m, "<object id=\"{}\" type=\"model\" name=\"{}\"><mesh><vertices>", k + 1, escape(name));
        for p in &body.mesh.positions {
            let _ = write!(m, "<vertex x=\"{}\" y=\"{}\" z=\"{}\"/>", crate::num(p.x), crate::num(p.y), crate::num(p.z));
        }
        m.push_str("</vertices><triangles>");
        for t in body.mesh.indices.as_chunks::<3>().0 {
            let _ = write!(m, "<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>", t[0], t[1], t[2]);
        }
        m.push_str("</triangles></mesh></object>\n");
    }
    m.push_str("</resources>\n<build>\n");
    for (_, mesh, at) in &scene.items {
        // Row-vector 3x4 matrix: the images of the x, y and z axes, then the translation.
        let m3 = at.matrix3;
        let t = [m3.x_axis, m3.y_axis, m3.z_axis, at.translation].iter().flat_map(|v| v.to_array()).map(crate::num).collect::<Vec<_>>().join(" ");
        let _ = writeln!(m, "<item objectid=\"{}\" transform=\"{t}\"/>", mesh + 1);
    }
    m.push_str("</build>\n</model>\n");
    zip_stored(&[("[Content_Types].xml", CONTENT_TYPES.as_bytes()), ("_rels/.rels", RELS.as_bytes()), ("3D/3dmodel.model", m.as_bytes())])
}

#[cfg(test)]
#[path = "threemf_out_tests.rs"]
mod tests;
