use super::*;
use cad_document::{PointKind, PointRef, Units};

fn pt(entity: u64) -> PointRef {
    PointRef { entity, kind: PointKind::Start }
}

#[test]
fn plain_constraints_have_short_labels() {
    let s = Settings::default();
    let all = [
        (Constraint::Horizontal(1), "H"),
        (Constraint::Vertical(1), "V"),
        (Constraint::Parallel(1, 2), "//"),
        (Constraint::Perpendicular(1, 2), "\u{22a5}"),
        (Constraint::Tangent(1, 2), "T"),
        (Constraint::Concentric(1, 2), "\u{25ce}"),
        (Constraint::Equal(1, 2), "="),
        (Constraint::Collinear(1, 2), "Col"),
        (Constraint::Symmetric(pt(1), pt(2), 3), "Sym"),
        (Constraint::Fixed { point: pt(1), at: cad_geometry::DVec2::ZERO }, "Fix"),
        (Constraint::Coincident(pt(1), pt(2)), "Coi"),
        (Constraint::PointOnLine(pt(1), 2), "On"),
        (Constraint::PointOnCircle(pt(1), 2), "On"),
    ];
    for (c, want) in all {
        assert_eq!(glyph_text(&c, &s), want);
    }
}

#[test]
fn valued_constraints_follow_the_document_settings() {
    let mm = Settings::default();
    assert_eq!(glyph_text(&Constraint::Distance(pt(1), pt(2), 40.0), &mm), "D 40.00");
    assert_eq!(glyph_text(&Constraint::Angle(1, 2, 30f64.to_radians()), &mm), "\u{2220} 30.00\u{b0}");
    assert_eq!(glyph_text(&Constraint::Radius(1, 5.0), &mm), "R 5.00");
    assert_eq!(glyph_text(&Constraint::Diameter(1, 10.0), &mm), "\u{d8} 10.00");
    let inch = Settings { units: Units::Inch, precision: 1 };
    assert_eq!(glyph_text(&Constraint::Distance(pt(1), pt(2), 25.4), &inch), "D 1.0");
    assert_eq!(glyph_text(&Constraint::Angle(1, 2, 30f64.to_radians()), &inch), "\u{2220} 30.0\u{b0}");
}

#[test]
fn a_driven_constraint_shows_its_expression_and_value() {
    let s = Settings::default();
    let d = Constraint::Distance(pt(1), pt(2), 40.0);
    assert_eq!(glyph_label(&d, Some("w"), &s), "D w = 40.00");
    assert_eq!(glyph_label(&d, None, &s), "D 40.00");
    assert_eq!(glyph_label(&Constraint::Angle(1, 2, 30f64.to_radians()), Some("a"), &s), "\u{2220} a = 30.00\u{b0}");
    assert_eq!(glyph_label(&Constraint::Horizontal(1), Some("w"), &s), "H");
}
