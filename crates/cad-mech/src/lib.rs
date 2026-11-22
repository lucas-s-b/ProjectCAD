//! Mechanical parts: gears, shafts, springs, ISO metric fasteners (cosmetic or modelled threads) and deep groove ball
//! bearings as solids from their parameters, and assembly math (joint frames, interference). Works on numbers,
//! knows no document.
mod bearing;
mod fastener;
mod gear;
mod interference;
mod joint;
mod material;
mod part;
mod shaft;
mod spring;

pub use bearing::{BEARINGS, Bearing, BearingSize, bearing};
pub use fastener::{Bolt, METRIC, Metric, Nut, THREAD_CLEARANCE, THREAD_SEGMENTS, basic_radius, metric, thread_radii};
pub use gear::{FLANK_SEGMENTS, Gear};
pub use interference::{CONTACT_GAP, CONTACT_VOLUME, Check, Clearance, Meeting, Overlap, check, clearance, overlap};
pub use joint::{face_frame, frame, mate};
pub use material::{MATERIALS, density};
pub use part::Part;
pub use shaft::Shaft;
pub use spring::{SEGMENTS_PER_TURN, Spring, WIRE_SIDES};

use cad_geometry::DVec2;
use cad_geometry::profile::{Loop, Seg};
use std::f64::consts::PI;

/// `x` with at most three decimals and no trailing zeros: 30 gives "30", 1.25 gives "1.25".
pub fn num(x: f64) -> String {
    let s = format!("{x:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A closed loop of lines through `points`; counter-clockwise points make an outer loop.
pub(crate) fn polygon(points: &[DVec2]) -> Loop {
    (0..points.len()).map(|k| Seg::Line { a: points[k], b: points[(k + 1) % points.len()] }).collect()
}

/// The circle of radius `r` about the origin as two half arcs: counter-clockwise (an outer loop) or clockwise (a hole).
pub(crate) fn circle(r: f64, ccw: bool) -> Loop {
    let l: Loop = [0.0, PI].map(|start| Seg::Arc { center: DVec2::ZERO, radius: r, start, sweep: PI }).to_vec();
    if ccw { l } else { l.iter().rev().map(Seg::reversed).collect() }
}

/// Corners of the regular hexagon with width across flats `s` about the origin, counter-clockwise from 30 degrees;
/// its flats face +x and -x.
pub(crate) fn hexagon(s: f64) -> Vec<DVec2> {
    (0..6).map(|k| DVec2::from_angle(PI / 6.0 + PI / 3.0 * k as f64) * (s / 3f64.sqrt())).collect()
}
