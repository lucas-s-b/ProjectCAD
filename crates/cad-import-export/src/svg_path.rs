//! SVG `path` data to shapes: lines and polylines stay exact, circular arcs become Arc, other curves are flattened.
use crate::svg_curve::{ArcGeom, arc_center, flatten_cubic, sample_arc};
use crate::svg_xform::Scan;
use cad_geometry::{DAffine2, DVec2, Shape};
use std::f64::consts::TAU;

enum Seg {
    Line(DVec2),
    Cubic(DVec2, DVec2, DVec2),
    Arc { rx: f64, ry: f64, rotation: f64, large: bool, sweep: bool, to: DVec2 },
}

struct Sub {
    start: DVec2,
    segs: Vec<Seg>,
    closed: bool,
}

/// Two numbers as a point, relative to `from` when `rel`.
fn point(s: &mut Scan, from: DVec2, rel: bool) -> Option<DVec2> {
    let p = DVec2::new(s.number()?, s.number()?);
    Some(if rel { from + p } else { p })
}

/// None when the data is malformed; everything before the error is dropped.
fn parse(d: &str) -> Option<Vec<Sub>> {
    let mut s = Scan::new(d);
    let mut subs: Vec<Sub> = Vec::new();
    let (mut cur, mut start) = (DVec2::ZERO, DVec2::ZERO);
    // second control point of the previous C/S (or the control of Q/T), for the smooth commands
    let mut ctrl: Option<(u8, DVec2)> = None;
    let mut cmd = 0u8;
    while let Some(c) = s.peek() {
        if c.is_ascii_alphabetic() {
            s.i += 1;
            cmd = c;
            if cmd == b'Z' || cmd == b'z' {
                subs.last_mut()?.closed = true;
                cur = start;
                ctrl = None;
                continue;
            }
        } else if cmd == 0 || cmd == b'Z' || cmd == b'z' {
            return None;
        } else if cmd == b'M' || cmd == b'm' {
            // further pairs after a move are lines
            cmd = if cmd == b'M' { b'L' } else { b'l' };
        }
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { cur } else { DVec2::ZERO };
        let mut next_ctrl = None;
        match cmd.to_ascii_uppercase() {
            b'M' => {
                cur = point(&mut s, cur, rel)?;
                start = cur;
                subs.push(Sub { start, segs: Vec::new(), closed: false });
            }
            b'L' | b'H' | b'V' => {
                cur = match cmd.to_ascii_uppercase() {
                    b'L' => point(&mut s, cur, rel)?,
                    b'H' => DVec2::new(base.x + s.number()?, cur.y),
                    _ => DVec2::new(cur.x, base.y + s.number()?),
                };
                subs.last_mut()?.segs.push(Seg::Line(cur));
            }
            b'C' | b'S' => {
                let c1 = if cmd.eq_ignore_ascii_case(&b'C') {
                    point(&mut s, cur, rel)?
                } else {
                    match ctrl {
                        Some((b'C', p)) => 2.0 * cur - p,
                        _ => cur,
                    }
                };
                let c2 = point(&mut s, cur, rel)?;
                let to = point(&mut s, cur, rel)?;
                subs.last_mut()?.segs.push(Seg::Cubic(c1, c2, to));
                next_ctrl = Some((b'C', c2));
                cur = to;
            }
            b'Q' | b'T' => {
                let q = if cmd.eq_ignore_ascii_case(&b'Q') {
                    point(&mut s, cur, rel)?
                } else {
                    match ctrl {
                        Some((b'Q', p)) => 2.0 * cur - p,
                        _ => cur,
                    }
                };
                let to = point(&mut s, cur, rel)?;
                // a quadratic is the cubic with these controls
                subs.last_mut()?.segs.push(Seg::Cubic(cur + (q - cur) * 2.0 / 3.0, to + (q - to) * 2.0 / 3.0, to));
                next_ctrl = Some((b'Q', q));
                cur = to;
            }
            b'A' => {
                let (rx, ry, rotation) = (s.number()?, s.number()?, s.number()?);
                let (large, sweep) = (s.flag()?, s.flag()?);
                let to = point(&mut s, cur, rel)?;
                subs.last_mut()?.segs.push(Seg::Arc { rx, ry, rotation, large, sweep, to });
                cur = to;
            }
            _ => return None,
        }
        ctrl = next_ctrl;
    }
    Some(subs)
}

/// A polyline run in world space; None when it collapses to fewer than two distinct points.
fn finish(mut pts: Vec<DVec2>, closed: bool) -> Option<Shape> {
    pts.dedup_by(|b, a| a.distance(*b) < 1e-12);
    if closed && pts.len() > 1 && pts[0].distance(pts[pts.len() - 1]) < 1e-12 {
        pts.pop();
    }
    match pts[..] {
        [a, b] => Some(Shape::Line { a, b }),
        _ if pts.len() > 2 => Some(Shape::Polyline { points: pts, closed }),
        _ => None,
    }
}

/// The Arc shape of a circular arc, when `m` keeps circles circular.
fn circular_arc(a: &ArcGeom, m: &DAffine2) -> Option<Shape> {
    if a.phi != 0.0 || (a.rx - a.ry).abs() > 1e-9 * a.rx {
        return None;
    }
    // the angle runs the other way when the SVG sweep flag is 0: the same arc counter-clockwise from its end
    let start = if a.delta >= 0.0 { a.theta } else { a.theta + a.delta };
    let arc = Shape::Arc { center: a.center, radius: a.rx, start: cad_geometry::shape::norm_angle(start), sweep: a.delta.abs().min(TAU) };
    arc.transformed(m).ok()
}

/// Shapes of a path in world space (`m` includes the y flip and the unit scale). None entries are geometry that cannot be a shape.
pub(crate) fn path_shapes(d: &str, m: &DAffine2) -> Option<Vec<Option<Shape>>> {
    let mut out = Vec::new();
    for sub in parse(d)? {
        let mut cur = sub.start;
        let mut run = vec![m.transform_point2(cur)];
        let mut split = false;
        for seg in &sub.segs {
            match seg {
                Seg::Line(p) => {
                    run.push(m.transform_point2(*p));
                    cur = *p;
                }
                Seg::Cubic(c1, c2, p) => {
                    flatten_cubic([run[run.len() - 1], m.transform_point2(*c1), m.transform_point2(*c2), m.transform_point2(*p)], &mut run);
                    cur = *p;
                }
                Seg::Arc { rx, ry, rotation, large, sweep, to } => {
                    match arc_center(cur, *to, *rx, *ry, *rotation, *large, *sweep) {
                        None => run.push(m.transform_point2(*to)),
                        Some(g) => match circular_arc(&g, m) {
                            Some(arc) => {
                                if run.len() > 1 {
                                    out.push(finish(std::mem::take(&mut run), false));
                                }
                                out.push(Some(arc));
                                run = vec![m.transform_point2(*to)];
                                split = true;
                            }
                            None => run.extend(sample_arc(&g, m).into_iter().skip(1)),
                        },
                    }
                    cur = *to;
                }
            }
        }
        // closing after an arc is an ordinary segment back to the start
        if sub.closed && split {
            run.push(m.transform_point2(sub.start));
        }
        if run.len() > 1 {
            out.push(finish(run, sub.closed && !split));
        }
    }
    Some(out)
}
