use super::*;

fn b(x0: f64, y0: f64, x1: f64, y1: f64) -> (DVec2, DVec2) {
    (DVec2::new(x0, y0), DVec2::new(x1, y1))
}

fn boxes() -> Vec<(DVec2, DVec2)> {
    vec![b(0.0, 0.0, 4.0, 2.0), b(10.0, 5.0, 12.0, 11.0), b(30.0, 1.0, 36.0, 3.0)]
}

fn xs(v: Vec<DVec2>) -> Vec<f64> {
    v.iter().map(|d| d.x).collect()
}
fn ys(v: Vec<DVec2>) -> Vec<f64> {
    v.iter().map(|d| d.y).collect()
}

#[test]
fn align_sides_and_centres_to_the_union_box() {
    // union box is (0,0)-(36,11)
    assert_eq!(xs(align_offsets(&boxes(), AlignMode::Left)), [0.0, -10.0, -30.0]);
    assert_eq!(xs(align_offsets(&boxes(), AlignMode::Right)), [32.0, 24.0, 0.0]);
    assert_eq!(ys(align_offsets(&boxes(), AlignMode::Top)), [9.0, 0.0, 8.0]);
    assert_eq!(ys(align_offsets(&boxes(), AlignMode::Bottom)), [0.0, -5.0, -1.0]);
    assert_eq!(xs(align_offsets(&boxes(), AlignMode::CentreX)), [16.0, 7.0, -15.0]);
    assert_eq!(ys(align_offsets(&boxes(), AlignMode::CentreY)), [4.5, -2.5, 3.5]);
}

#[test]
fn distribute_spaces_the_centres_evenly_and_keeps_the_outer_two() {
    // centres x: 2, 11, 33 -> 2, 17.5, 33
    assert_eq!(xs(align_offsets(&boxes(), AlignMode::DistributeX)), [0.0, 6.5, 0.0]);
    // centres y: 1, 8, 2: sorted 1 (0), 2 (2), 8 (1) -> middle one goes to 4.5
    assert_eq!(ys(align_offsets(&boxes(), AlignMode::DistributeY)), [0.0, 0.0, 2.5]);
}

#[test]
fn distribute_ignores_input_order_and_small_selections() {
    let mut v = boxes();
    v.reverse();
    assert_eq!(xs(align_offsets(&v, AlignMode::DistributeX)), [0.0, 6.5, 0.0]);
    assert_eq!(align_offsets(&boxes()[..2], AlignMode::DistributeX), [DVec2::ZERO; 2]);
    assert!(align_offsets(&[], AlignMode::Left).is_empty());
}
