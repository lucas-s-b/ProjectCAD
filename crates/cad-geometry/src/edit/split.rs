use super::param::{domain, not_curve, param_of, point_at, sub};
use crate::shape::{GeomError, Shape};
use glam::DVec2;

const AT_END: GeomError = GeomError::Degenerate("split point is at an end");

/// Cuts a curve in two at the point of it nearest to `p`. Closed curves (circle, closed polyline, rect) are opened there and stay one shape.
pub fn split(shape: &Shape, p: DVec2) -> Result<Vec<Shape>, GeomError> {
    let Some((end, closed)) = domain(shape) else {
        return Err(not_curve(shape));
    };
    let u = param_of(shape, p);
    if closed {
        return sub(shape, u, u + end).map(|s| vec![s]).ok_or(AT_END);
    }
    let near_end = |at: f64| point_at(shape, u).distance(point_at(shape, at)) <= 1e-9;
    if near_end(0.0) || near_end(end) {
        return Err(AT_END);
    }
    match (sub(shape, 0.0, u), sub(shape, u, end)) {
        (Some(a), Some(b)) => Ok(vec![a, b]),
        _ => Err(AT_END),
    }
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod tests;
