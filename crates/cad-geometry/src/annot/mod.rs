mod extra;

pub use extra::{hatch_lines, inside_polygon};
use crate::shape::Shape;
use glam::DVec2;
use std::f64::consts::{FRAC_PI_2, PI};

/// Corners of the text box: baseline-left, baseline-right, top-right, top-left.
/// shortcut: width is estimated as 0.6 * height per character, replace with real shaping when cosmic-text is added
pub fn text_box(pos: DVec2, height: f64, angle: f64, text: &str) -> [DVec2; 4] {
    let w = 0.6 * height * text.chars().count() as f64;
    let (u, v) = (DVec2::from_angle(angle), DVec2::from_angle(angle + FRAC_PI_2));
    [pos, pos + u * w, pos + u * w + v * height, pos + v * height]
}

pub struct DimLayout {
    /// Extension lines, from the measured points to 0.5 * text_height past the dimension line.
    pub ext: [[DVec2; 2]; 2],
    pub line: [DVec2; 2],
    /// Filled arrow heads: tip on the extension line, length text_height, half-width text_height / 6.
    pub arrows: [[DVec2; 3]; 2],
    /// 0.9 * text_height from the dimension line, on the side away from a-b, at the middle of the line.
    pub text_center: DVec2,
    /// Direction of a->b, flipped by PI when it would read upside-down; kept in (-PI/2, PI/2].
    pub text_angle: f64,
    /// Distance a-b in millimetres.
    pub value: f64,
}

pub fn dim_layout(a: DVec2, b: DVec2, offset: f64, text_height: f64) -> DimLayout {
    let value = a.distance(b);
    let dir = (b - a).normalize_or_zero();
    let side = dir.perp() * if offset < 0.0 { -1.0 } else { 1.0 };
    let (p, q) = (a + dir.perp() * offset, b + dir.perp() * offset);
    let past = side * 0.5 * text_height;
    let arrow = |tip: DVec2, along: DVec2| {
        let base = tip + along * text_height;
        let w = dir.perp() * text_height / 6.0;
        [tip, base + w, base - w]
    };
    let mut text_angle = dir.y.atan2(dir.x);
    if text_angle > FRAC_PI_2 + 1e-9 {
        text_angle -= PI;
    } else if text_angle <= -FRAC_PI_2 + 1e-9 {
        text_angle += PI;
    }
    DimLayout {
        ext: [[a, p + past], [b, q + past]],
        line: [p, q],
        arrows: [arrow(p, dir), arrow(q, -dir)],
        text_center: (p + q) / 2.0 + side * 0.9 * text_height,
        text_angle,
        value,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LabelText {
    Literal(String),
    Length(f64),
    Radius(f64),
    Diameter(f64),
    /// Radians.
    Angle(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Align {
    /// The anchor is the baseline start.
    Left,
    /// The anchor is the middle of the text box.
    Center,
    /// The anchor is the baseline end.
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub text: LabelText,
    pub anchor: DVec2,
    pub align: Align,
    pub angle: f64,
    pub height: f64,
}

pub fn labels(shape: &Shape) -> Vec<Label> {
    match shape {
        Shape::Text { pos, height, angle, text } => {
            vec![Label { text: LabelText::Literal(text.clone()), anchor: *pos, align: Align::Left, angle: *angle, height: *height }]
        }
        Shape::Dimension { a, b, offset, text_height } => {
            let l = dim_layout(*a, *b, *offset, *text_height);
            vec![Label { text: LabelText::Length(l.value), anchor: l.text_center, align: Align::Center, angle: l.text_angle, height: *text_height }]
        }
        _ => extra::parts(shape).map(|p| p.labels).unwrap_or_default(),
    }
}

/// Filled triangles (arrow heads).
pub fn fills(shape: &Shape) -> Vec<[DVec2; 3]> {
    match shape {
        Shape::Dimension { a, b, offset, text_height } => dim_layout(*a, *b, *offset, *text_height).arrows.to_vec(),
        _ => extra::parts(shape).map(|p| p.fills).unwrap_or_default(),
    }
}

/// XY polylines to stroke: the outline without the text box of a Text and without arrow outlines.
pub fn strokes(shape: &Shape) -> Vec<Vec<DVec2>> {
    match shape {
        Shape::Text { .. } => Vec::new(),
        Shape::Hatch { .. } | Shape::Leader { .. } | Shape::RadialDim { .. } | Shape::AngularDim { .. } => {
            extra::parts(shape).map(|p| p.strokes).unwrap_or_default()
        }
        Shape::Dimension { a, b, offset, text_height } => {
            let l = dim_layout(*a, *b, *offset, *text_height);
            l.ext.iter().chain([&l.line]).map(|[p, q]| vec![*p, *q]).collect()
        }
        _ => crate::mesh::outline_paths(shape).iter().map(|p| p.iter().map(|v| v.truncate()).collect()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v2(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }
    fn close(a: DVec2, b: DVec2) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn text_box_size_and_rotation() {
        let b = text_box(v2(1.0, 2.0), 10.0, 0.0, "abc");
        assert!(close(b[0], v2(1.0, 2.0)) && close(b[2], v2(19.0, 12.0)));
        let r = text_box(v2(0.0, 0.0), 10.0, FRAC_PI_2, "abc");
        assert!(close(r[1], v2(0.0, 18.0)) && close(r[3], v2(-10.0, 0.0)));
    }

    #[test]
    fn layout_positive_offset() {
        let l = dim_layout(v2(0.0, 0.0), v2(100.0, 0.0), 20.0, 4.0);
        assert!(close(l.line[0], v2(0.0, 20.0)) && close(l.line[1], v2(100.0, 20.0)));
        assert_eq!(l.value, 100.0);
        assert!(close(l.ext[0][0], v2(0.0, 0.0)) && close(l.ext[0][1], v2(0.0, 22.0)));
        assert!(close(l.ext[1][1], v2(100.0, 22.0)));
        assert!(close(l.arrows[0][0], v2(0.0, 20.0)) && close(l.arrows[1][0], v2(100.0, 20.0)));
        assert!(close(l.arrows[0][1], v2(4.0, 20.0 + 4.0 / 6.0)));
        assert!(close(l.text_center, v2(50.0, 23.6)) && l.text_angle == 0.0);
    }

    #[test]
    fn layout_negative_offset_and_flip() {
        let l = dim_layout(v2(0.0, 0.0), v2(100.0, 0.0), -20.0, 4.0);
        assert!(close(l.line[0], v2(0.0, -20.0)) && close(l.text_center, v2(50.0, -23.6)));
        assert!(close(l.ext[0][1], v2(0.0, -22.0)));
        for (b, ang) in [(v2(-100.0, 0.0), 0.0), (v2(0.0, -50.0), FRAC_PI_2), (v2(-10.0, -10.0), PI / 4.0)] {
            let l = dim_layout(v2(0.0, 0.0), b, 5.0, 4.0);
            assert!(l.text_angle > -FRAC_PI_2 && l.text_angle <= FRAC_PI_2 + 1e-9);
            assert!((l.text_angle - ang).abs() < 1e-9, "{b:?}");
        }
    }
}
