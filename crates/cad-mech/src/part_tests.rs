use super::*;
use crate::{Bolt, Gear, Nut, Shaft, Spring, metric};

#[test]
fn designations_and_threads() {
    let m8 = metric("M8").unwrap();
    let parts = [
        (Part::Gear(Gear { module: 2.0, teeth: 20, pressure: 20f64.to_radians(), width: 10.0, bore: 8.0 }), "Spur gear m2 z20 b10", None),
        (Part::Shaft(Shaft { steps: vec![(20.0, 30.0), (16.0, 50.0)], chamfer: 1.0 }), "Shaft \u{d8}20x80", None),
        (Part::Spring(Spring { wire: 2.0, mean: 20.0, pitch: 5.0, turns: 6.0 }), "Compression spring d2 D20 L32", None),
        (Part::Bolt(Bolt { size: m8, length: 30.0 }), "ISO 4017 M8x30", Some("M8x1.25".to_string())),
        (Part::Nut(Nut { size: m8 }), "ISO 4032 M8", Some("M8x1.25".to_string())),
    ];
    for (p, designation, thread) in parts {
        assert_eq!(p.designation(), designation);
        assert_eq!(p.thread(), thread);
    }
}

#[test]
fn part_body_is_the_generator_body() {
    let n = Nut { size: metric("M8").unwrap() };
    let a = Part::Nut(n).body(&Plane3::XY, 0.02).unwrap().volume();
    assert_eq!(a, n.body(&Plane3::XY, 0.02).unwrap().volume());
    let s = Part::Spring(Spring { wire: 2.0, mean: 20.0, pitch: 1.0, turns: 6.0 });
    assert_eq!(s.body(&Plane3::XY, 0.02), Err(ModelError::Invalid("spring pitch must be larger than the wire diameter")));
}
