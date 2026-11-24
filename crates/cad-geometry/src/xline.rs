//! Construction lines (`Shape::XLine`): clipping to a box for drawing, and the parallel snap.
use glam::DVec2;

/// The part of the line through `point` along the unit `dir` (from `point` on only, for a ray) inside the box
/// `lo..hi`; None when it misses the box.
pub fn clip(point: DVec2, dir: DVec2, ray: bool, lo: DVec2, hi: DVec2) -> Option<(DVec2, DVec2)> {
    let (mut t0, mut t1) = (if ray { 0.0 } else { f64::NEG_INFINITY }, f64::INFINITY);
    for (p, d, a, b) in [(point.x, dir.x, lo.x, hi.x), (point.y, dir.y, lo.y, hi.y)] {
        if d.abs() < 1e-15 {
            if p < a || p > b {
                return None;
            }
            continue;
        }
        let (u, v) = ((a - p) / d, (b - p) / d);
        t0 = t0.max(u.min(v));
        t1 = t1.min(u.max(v));
    }
    (t0 <= t1).then(|| (point + dir * t0, point + dir * t1))
}

/// Parallel snap: from `from`, a cursor `cursor` whose direction is within `max_angle` (radians) of `dir` or its
/// reverse snaps onto the line from `from` along `dir`. None when not close enough or too near `from`.
pub fn parallel(from: DVec2, cursor: DVec2, dir: DVec2, max_angle: f64) -> Option<DVec2> {
    let v = cursor - from;
    let len = v.length();
    if len < 1e-9 || dir.length() < 1e-12 {
        return None;
    }
    let d = dir.normalize();
    let along = v.dot(d);
    let sin = (d.perp_dot(v) / len).abs();
    (sin <= max_angle.sin()).then(|| from + d * along)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_lines_and_rays_to_a_box() {
        let (lo, hi) = (DVec2::splat(-10.0), DVec2::splat(10.0));
        let (a, b) = clip(DVec2::ZERO, DVec2::X, false, lo, hi).unwrap();
        assert_eq!((a, b), (DVec2::new(-10.0, 0.0), DVec2::new(10.0, 0.0)));
        let (a, b) = clip(DVec2::ZERO, DVec2::X, true, lo, hi).unwrap();
        assert_eq!((a, b), (DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let diag = DVec2::ONE.normalize();
        let (a, b) = clip(DVec2::new(0.0, -5.0), diag, false, lo, hi).unwrap();
        assert!(a.distance(DVec2::new(-5.0, -10.0)) < 1e-9 && b.distance(DVec2::new(10.0, 5.0)) < 1e-9);
        assert_eq!(clip(DVec2::new(0.0, 20.0), DVec2::X, false, lo, hi), None);
        // A ray pointing away from the box misses it.
        assert_eq!(clip(DVec2::new(20.0, 0.0), DVec2::X, true, lo, hi), None);
    }

    #[test]
    fn parallel_snap_projects_onto_the_direction() {
        let p = parallel(DVec2::ZERO, DVec2::new(10.0, 0.2), DVec2::X, 2f64.to_radians()).unwrap();
        assert!(p.distance(DVec2::new(10.0, 0.0)) < 1e-12);
        let back = parallel(DVec2::ZERO, DVec2::new(-10.0, 0.2), DVec2::X, 2f64.to_radians()).unwrap();
        assert!(back.distance(DVec2::new(-10.0, 0.0)) < 1e-12);
        assert_eq!(parallel(DVec2::ZERO, DVec2::new(10.0, 3.0), DVec2::X, 2f64.to_radians()), None);
    }
}
