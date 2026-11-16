use glam::DVec2;

/// Point at angle `t` (radians around the ellipse); `angle` is the direction of the `rx` axis.
pub fn ellipse_point(center: DVec2, rx: f64, ry: f64, angle: f64, t: f64) -> DVec2 {
    center + DVec2::from_angle(angle).rotate(DVec2::new(rx * t.cos(), ry * t.sin()))
}

pub fn spline_spans(points: &[DVec2], closed: bool) -> usize {
    if closed { points.len() } else { points.len().saturating_sub(1) }
}

/// Point on span `span` (between points[span] and points[span + 1], wrapping when closed), u in 0..=1.
/// Centripetal Catmull-Rom (Barry-Goldman): knots advance by sqrt(distance). Open ends use the mirrored
/// phantom points 2*P0 - P1 and 2*Pn - Pn-1.
pub fn spline_point(points: &[DVec2], closed: bool, span: usize, u: f64) -> DVec2 {
    let n = points.len();
    let at = |k: isize| -> DVec2 {
        if closed {
            points[k.rem_euclid(n as isize) as usize]
        } else if k < 0 {
            2.0 * points[0] - points[1]
        } else if k as usize >= n {
            2.0 * points[n - 1] - points[n - 2]
        } else {
            points[k as usize]
        }
    };
    let s = span as isize;
    let p = [at(s - 1), at(s), at(s + 1), at(s + 2)];
    let mut t = [0.0; 4];
    for k in 1..4 {
        t[k] = t[k - 1] + p[k].distance(p[k - 1]).sqrt();
    }
    let x = t[1] + u * (t[2] - t[1]);
    let lerp = |a: DVec2, b: DVec2, ta: f64, tb: f64| a * ((tb - x) / (tb - ta)) + b * ((x - ta) / (tb - ta));
    let a1 = lerp(p[0], p[1], t[0], t[1]);
    let a2 = lerp(p[1], p[2], t[1], t[2]);
    let a3 = lerp(p[2], p[3], t[2], t[3]);
    let b1 = lerp(a1, a2, t[0], t[2]);
    let b2 = lerp(a2, a3, t[1], t[3]);
    lerp(b1, b2, t[1], t[2])
}

/// Flattened spline: `per_span` segments per span.
pub fn spline_path(points: &[DVec2], closed: bool, per_span: usize) -> Vec<DVec2> {
    let mut out = Vec::new();
    for span in 0..spline_spans(points, closed) {
        out.extend((0..per_span).map(|i| spline_point(points, closed, span, i as f64 / per_span as f64)));
    }
    out.push(if closed { points[0] } else { points[points.len() - 1] });
    out
}

#[cfg(test)]
#[path = "curve_tests.rs"]
mod tests;
