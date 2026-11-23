//! The position equation of a planar four-bar linkage: the reference that closed-loop joints are validated against.

/// Benchmark case of `Accuracy::Validated` for the four-bar linkage.
pub const FOUR_BAR: &str = "four-bar position analysis";

/// Output (rocker) angle θ4 in (−π, π] of the planar four-bar with ground pivots A = (0, 0) and D = (d, 0), crank AB
/// of length `a` at angle `theta2` (radians from +x), coupler BC of length `b` and rocker DC of length `c`, θ4 measured
/// at D from +x. With B = a (cos θ2, sin θ2), r = |B − D|, φ = atan2(B − D) and the triangle angle α at D,
/// cos α = (c² + r² − b²) / (2 c r): θ4 = φ − α for `branch` 1, φ + α for −1. Satisfies Freudenstein's equation
/// K1 cos θ4 − K2 cos θ2 + K3 = cos(θ2 − θ4), K1 = d/a, K2 = d/c, K3 = (a² − b² + c² + d²) / (2ac).
/// None when the links cannot meet at this crank angle (r = 0, r > b + c or r < |b − c|).
pub fn four_bar_rocker(a: f64, b: f64, c: f64, d: f64, theta2: f64, branch: f64) -> Option<f64> {
    let (bx, by) = (a * theta2.cos(), a * theta2.sin());
    let r = (bx - d).hypot(by);
    if r == 0.0 || r > b + c || r < (b - c).abs() {
        return None;
    }
    let alpha = ((c * c + r * r - b * b) / (2.0 * c * r)).clamp(-1.0, 1.0).acos();
    let t = by.atan2(bx - d) - branch * alpha;
    Some(t.sin().atan2(t.cos()))
}

#[cfg(test)]
#[path = "four_bar_tests.rs"]
mod tests;
