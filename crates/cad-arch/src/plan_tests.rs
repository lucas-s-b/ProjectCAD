use super::*;
use crate::fixtures::*;

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}
fn length(p: &Plan) -> f64 {
    p.edges.iter().map(|(_, [a, b])| a.distance(*b)).sum()
}
fn filled(p: &Plan) -> f64 {
    p.fill.iter().map(|(_, [a, b, c])| (*b - *a).perp_dot(*c - *a).abs() / 2.0).sum()
}

#[test]
fn single_wall_outline_is_its_rectangle() {
    let p = plan(&[wall((0.0, 0.0), (4000.0, 0.0), 200.0)]);
    assert_eq!(p.edges.len(), 4);
    close(length(&p), 8400.0);
    assert_eq!(p.fill.len(), 2);
    close(filled(&p), 800000.0);
}

#[test]
fn l_outline_has_no_mitre_line() {
    let p = plan(&[wall((0.0, 0.0), (4000.0, 0.0), 200.0), wall((4000.0, 0.0), (4000.0, 3000.0), 200.0)]);
    assert_eq!(p.edges.len(), 6);
    close(length(&p), 14400.0);
    close(filled(&p), 1400000.0);
    let (a, b) = (v(4100.0, -100.0), v(3900.0, 100.0));
    let is = |q: DVec2, r: DVec2| q.distance(a) <= 1e-6 && r.distance(b) <= 1e-6;
    assert!(!p.edges.iter().any(|(_, [q, r])| is(*q, *r) || is(*r, *q)));
}

#[test]
fn t_outline_opens_the_through_wall() {
    let p = plan(&[wall((0.0, 0.0), (4000.0, 0.0), 200.0), wall((2000.0, 0.0), (2000.0, 3000.0), 100.0)]);
    assert_eq!(p.edges.len(), 8);
    close(length(&p), 14200.0);
    close(p.edges.iter().filter(|(i, _)| *i == 0).map(|(_, [a, b])| a.distance(*b)).sum::<f64>(), 8300.0);
}

#[test]
fn x_outline_is_a_cross() {
    let p = plan(&[wall((0.0, 0.0), (4000.0, 0.0), 200.0), wall((2000.0, -2000.0), (2000.0, 2000.0), 200.0)]);
    assert_eq!(p.edges.len(), 12);
    close(length(&p), 16000.0);
    close(filled(&p), 1600000.0);
}

#[test]
fn three_way_outline() {
    let p = plan(&[
        wall((0.0, 0.0), (2000.0, 0.0), 200.0),
        wall((0.0, 0.0), (0.0, 2000.0), 200.0),
        wall((0.0, 0.0), (-2000.0, 0.0), 200.0),
    ]);
    assert_eq!(p.edges.len(), 9);
    close(length(&p), 12200.0);
    close(filled(&p), 1180000.0);
}
