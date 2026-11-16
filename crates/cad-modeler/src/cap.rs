use crate::ModelError;
use glam::DVec2;
use std::collections::HashMap;

const BAD: ModelError = ModelError::Invalid("cannot triangulate a cap");

/// Triangles (indices into the concatenation of `rings`, the outer ring first) that fill the polygon with its holes.
/// Their winding is arbitrary. earcut drops collinear boundary points, which would leave T-junctions, so every dropped
/// point is put back by fanning the triangle that owns its boundary edge.
pub(crate) fn triangulate(rings: &[Vec<DVec2>]) -> Result<Vec<[usize; 3]>, ModelError> {
    let (mut data, mut holes, mut starts) = (Vec::new(), Vec::new(), Vec::new());
    for (i, r) in rings.iter().enumerate() {
        if i > 0 {
            holes.push(data.len() / 2);
        }
        starts.push(data.len() / 2);
        data.extend(r.iter().flat_map(|p| [p.x, p.y]));
    }
    let total = data.len() / 2;
    let flat = earcutr::earcut(&data, &holes, 2).map_err(|_| BAD)?;
    let mut used = vec![false; total];
    for &i in &flat {
        used[i] = true;
    }
    // A boundary edge between two kept points, in ring direction, with the dropped points on it.
    let mut dropped: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (ring, &start) in rings.iter().zip(&starts) {
        let n = ring.len();
        let first = (0..n).find(|&k| used[start + k]).ok_or(BAD)?;
        let (mut prev, mut run) = (start + first, Vec::new());
        for k in 1..=n {
            let idx = start + (first + k) % n;
            if used[idx] {
                if !run.is_empty() {
                    dropped.insert((prev, idx), std::mem::take(&mut run));
                }
                prev = idx;
            } else {
                run.push(idx);
            }
        }
    }
    let (mut todo, mut out): (Vec<[usize; 3]>, Vec<[usize; 3]>) = (flat.as_chunks::<3>().0.to_vec(), Vec::new());
    while let Some(t) = todo.pop() {
        let split = (0..3).find_map(|k| {
            let (x, y) = (t[k], t[(k + 1) % 3]);
            let along = dropped.get(&(x, y)).cloned().or_else(|| dropped.get(&(y, x)).map(|l| l.iter().rev().copied().collect()))?;
            Some((k, along))
        });
        let Some((k, along)) = split else {
            out.push(t);
            continue;
        };
        let (x, y, w) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
        let path: Vec<usize> = std::iter::once(x).chain(along).chain(std::iter::once(y)).collect();
        todo.extend(path.windows(2).map(|p| [p[0], p[1], w]));
    }
    if out.len() + 2 != total + 2 * holes.len() {
        return Err(BAD);
    }
    Ok(out)
}
