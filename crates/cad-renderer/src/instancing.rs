//! Instanced drawing: a mesh's triangles and edges are uploaded once and drawn under any number of placements.
use crate::Vertex;
use std::ops::Range;

/// One mesh in its own coordinates: triangle corners (white with alpha 1; the instance colours them) and edge line
/// ends (zero normal, their own colour).
#[derive(Debug, Clone, Default)]
pub struct MeshData {
    pub tris: Vec<Vertex>,
    pub lines: Vec<Vertex>,
}

/// Per-instance vertex data (step mode Instance, shader locations 3 to 7).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    /// Column-major mesh-to-world matrix; a rigid motion, so it also turns the normals.
    pub model: [[f32; 4]; 4],
    /// Multiplies the triangle colour; alpha 1 lets the section plane clip the instance. Lines keep their own colour.
    pub color: [f32; 4],
}

/// One placement of the mesh with key `mesh`; `faces` false draws its edges only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Draw {
    pub mesh: u64,
    pub instance: Instance,
    pub faces: bool,
}

/// A run of instances of one mesh with the same `faces`.
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    pub mesh: u64,
    pub range: Range<u32>,
    pub faces: bool,
}

/// Orders `draws` by mesh key, faces before edges-only, otherwise keeping their order, into one instance array and
/// its batches.
pub fn batch(draws: &[Draw]) -> (Vec<Instance>, Vec<Batch>) {
    let mut order: Vec<&Draw> = draws.iter().collect();
    order.sort_by_key(|d| (d.mesh, !d.faces));
    let mut batches: Vec<Batch> = Vec::new();
    for (i, d) in order.iter().enumerate() {
        let i = i as u32;
        match batches.last_mut() {
            Some(b) if b.mesh == d.mesh && b.faces == d.faces => b.range.end = i + 1,
            _ => batches.push(Batch { mesh: d.mesh, range: i..i + 1, faces: d.faces }),
        }
    }
    (order.iter().map(|d| d.instance).collect(), batches)
}

#[cfg(test)]
#[path = "instancing_tests.rs"]
mod tests;
