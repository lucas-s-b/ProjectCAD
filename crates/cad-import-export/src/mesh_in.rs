//! STL (binary or ASCII) and OBJ import of triangle meshes, in millimetres: points welded by exact value, triangles
//! as point indices. Whether the mesh is closed is checked by `Body::from_mesh`.
use std::collections::HashMap;

/// Points and triangles of an imported mesh (`FeatureKind::Mesh`).
pub type MeshData = (Vec<[f32; 3]>, Vec<[u32; 3]>);

/// Points welded by exact `f32` value, and the triangles between them.
#[derive(Default)]
struct Welder {
    index: HashMap<[u32; 3], u32>,
    points: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
}

impl Welder {
    fn point(&mut self, p: [f32; 3]) -> u32 {
        // -0.0 and 0.0 differ in bits but are one point.
        let p = p.map(|v| if v == 0.0 { 0.0 } else { v });
        let next = self.points.len() as u32;
        let id = *self.index.entry(p.map(f32::to_bits)).or_insert(next);
        if id == next {
            self.points.push(p);
        }
        id
    }

    /// Skips a triangle with a repeated point.
    fn triangle(&mut self, t: [u32; 3]) {
        if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
            self.triangles.push(t);
        }
    }

    fn finish(self) -> Result<MeshData, String> {
        if self.triangles.is_empty() { Err("no triangles".into()) } else { Ok((self.points, self.triangles)) }
    }
}

const TRUNCATED: &str = "STL is empty or truncated";

/// Binary STL when the size is 84 + 50 x the triangle count at byte 80, otherwise ASCII (`facet` / `vertex x y z`).
/// Points with the same `f32` value become one; triangles with a repeated point are dropped.
pub fn from_stl(bytes: &[u8]) -> Result<MeshData, String> {
    let mut w = Welder::default();
    let count = bytes.get(80..84).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as usize);
    if count.is_some_and(|n| bytes.len() as u64 == 84 + 50 * n as u64) {
        let float = |b: &[u8]| f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        for facet in bytes[84..].as_chunks::<50>().0 {
            // 12 bytes of normal, then three corners.
            let ids = [0, 1, 2].map(|k| {
                let at = 12 + 12 * k;
                w.point([float(&facet[at..]), float(&facet[at + 4..]), float(&facet[at + 8..])])
            });
            w.triangle(ids);
        }
        return w.finish();
    }
    let text = String::from_utf8_lossy(bytes);
    if !text.trim_start().starts_with("solid") {
        return Err(TRUNCATED.into());
    }
    let mut corners = Vec::new();
    let mut words = text.split_whitespace();
    while let Some(word) = words.next() {
        if word.eq_ignore_ascii_case("vertex") {
            let mut p = [0.0f32; 3];
            for v in &mut p {
                *v = words.next().ok_or(TRUNCATED)?.parse().map_err(|_| "STL has a coordinate that is not a number")?;
            }
            corners.push(w.point(p));
        }
    }
    if corners.len() % 3 != 0 {
        return Err(TRUNCATED.into());
    }
    for t in corners.as_chunks::<3>().0 {
        w.triangle([t[0], t[1], t[2]]);
    }
    w.finish()
}

/// `v x y z` and `f a b c ...` lines (`a`, `a/t`, `a//n`, `a/t/n`; negative indices count back from the last point);
/// polygons are split into a fan from their first corner. Every other line is ignored.
pub fn from_obj(text: &str) -> Result<MeshData, String> {
    let mut w = Welder::default();
    // OBJ point number (0-based) to welded point.
    let mut map: Vec<u32> = Vec::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("v") => {
                let mut p = [0.0f32; 3];
                for v in &mut p {
                    *v = words.next().and_then(|s| s.parse::<f64>().ok()).ok_or("OBJ has a coordinate that is not a number")? as f32;
                }
                map.push(w.point(p));
            }
            Some("f") => {
                let mut ids = Vec::new();
                for word in words {
                    let n: i64 = word.split('/').next().and_then(|s| s.parse().ok()).ok_or("OBJ face has an index that is not a number")?;
                    let at = if n < 0 { map.len() as i64 + n } else { n - 1 };
                    let id = usize::try_from(at).ok().and_then(|k| map.get(k)).ok_or_else(|| format!("OBJ face refers to point {n} of {}", map.len()))?;
                    ids.push(*id);
                }
                for k in 2..ids.len() {
                    w.triangle([ids[0], ids[k - 1], ids[k]]);
                }
            }
            _ => {}
        }
    }
    w.finish()
}

#[cfg(test)]
#[path = "mesh_in_tests.rs"]
mod tests;
