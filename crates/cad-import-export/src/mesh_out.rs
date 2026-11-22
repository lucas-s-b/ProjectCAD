//! STL and OBJ export of solid bodies, in millimetres.
use cad_geometry::DVec3;
use cad_modeler::Body;
use std::fmt::Write;

const STL_HEADER: &[u8] = b"ProjectCAD binary STL, units mm";

/// Unit normal by the right-hand rule; zero for a degenerate triangle.
fn facet_normal([a, b, c]: [DVec3; 3]) -> DVec3 {
    (b - a).cross(c - a).try_normalize().unwrap_or(DVec3::ZERO)
}

/// Binary STL of all bodies: 80-byte header, u32 triangle count, then 50 bytes per triangle.
pub fn to_stl(bodies: &[&Body]) -> Vec<u8> {
    let count: usize = bodies.iter().map(|b| b.mesh.triangle_count()).sum();
    let mut out = vec![0u8; 80];
    out[..STL_HEADER.len()].copy_from_slice(STL_HEADER);
    out.extend_from_slice(&(count as u32).to_le_bytes());
    for body in bodies {
        let m = &body.mesh;
        for t in m.indices.as_chunks::<3>().0 {
            let tri = t.map(|i| m.positions[i as usize]);
            for v in std::iter::once(facet_normal(tri)).chain(tri) {
                for c in [v.x, v.y, v.z] {
                    out.extend_from_slice(&(c as f32).to_le_bytes());
                }
            }
            out.extend_from_slice(&[0, 0]);
        }
    }
    out
}

/// OBJ text with one `o <name>` group per body; face indices are 1-based and global.
pub fn to_obj(bodies: &[(&str, &Body)]) -> String {
    let mut out = String::from("# ProjectCAD OBJ, units mm\n");
    let mut base = 1;
    for (name, body) in bodies {
        let m = &body.mesh;
        let _ = writeln!(out, "o {name}");
        for p in &m.positions {
            let _ = writeln!(out, "v {} {} {}", p.x, p.y, p.z);
        }
        for t in m.indices.as_chunks::<3>().0 {
            let _ = writeln!(out, "f {} {} {}", base + t[0] as usize, base + t[1] as usize, base + t[2] as usize);
        }
        base += m.positions.len();
    }
    out
}

#[cfg(test)]
#[path = "mesh_out_tests.rs"]
mod tests;
