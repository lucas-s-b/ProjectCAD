use crate::shape::{GeomError, Shape};
use crate::tol::Tolerance;
use glam::DVec2;

const JOIN_TOL: f64 = 1e-6;
const NOT_ONE_CHAIN: GeomError = GeomError::Degenerate("shapes do not form one chain");

fn path(shape: &Shape) -> Result<Vec<DVec2>, GeomError> {
    match shape {
        Shape::Line { a, b } => Ok(vec![*a, *b]),
        Shape::Polyline { points, closed: false } => Ok(points.clone()),
        _ => Err(GeomError::Unsupported("only lines and open polylines can be joined")),
    }
}

/// Joins lines and open polylines that touch end to end (within 1e-6) into one polyline (closed when the chain closes).
pub fn join(shapes: &[&Shape]) -> Result<Shape, GeomError> {
    let paths = shapes.iter().map(|s| path(s)).collect::<Result<Vec<_>, _>>()?;
    let n = paths.len();
    if n == 0 {
        return Err(NOT_ONE_CHAIN);
    }
    // End `e` of piece `i`: 0 is its first point, 1 its last.
    let end = |i: usize, e: usize| if e == 0 { paths[i][0] } else { paths[i][paths[i].len() - 1] };
    // The one end of another piece that touches end `e` of piece `i`; two or more is a branch.
    let touching = |i: usize, e: usize| -> Result<Option<(usize, usize)>, GeomError> {
        let hits: Vec<(usize, usize)> = (0..n)
            .filter(|&j| j != i)
            .flat_map(|j| [(j, 0), (j, 1)])
            .filter(|&(j, f)| end(j, f).distance(end(i, e)) <= JOIN_TOL)
            .collect();
        if hits.len() > 1 { Err(NOT_ONE_CHAIN) } else { Ok(hits.first().copied()) }
    };
    let link = (0..n).map(|i| Ok([touching(i, 0)?, touching(i, 1)?])).collect::<Result<Vec<_>, GeomError>>()?;
    // An open chain starts at a free end; a closed one can start anywhere.
    let (mut i, mut e) = (0..n).flat_map(|i| [(i, 0), (i, 1)]).find(|&(i, e)| link[i][e].is_none()).unwrap_or((0, 0));
    let (mut points, mut seen, mut closed) = (Vec::new(), vec![false; n], false);
    loop {
        seen[i] = true;
        let mut p = paths[i].clone();
        if e == 1 {
            p.reverse();
        }
        points.extend(if points.is_empty() { &p[..] } else { &p[1..] });
        match link[i][1 - e] {
            Some((j, f)) if !seen[j] => (i, e) = (j, f),
            Some(_) => {
                closed = true;
                break;
            }
            None => break,
        }
    }
    if seen.contains(&false) {
        return Err(NOT_ONE_CHAIN);
    }
    if closed {
        points.pop();
    }
    let joined = Shape::Polyline { points, closed };
    joined.validate(&Tolerance::default())?;
    Ok(joined)
}

#[cfg(test)]
#[path = "join_tests.rs"]
mod tests;
