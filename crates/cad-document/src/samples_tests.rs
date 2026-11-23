use super::*;
use crate::Rebuilder;

#[test]
fn house_rebuilds_every_element() {
    let d = house();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.walls.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [1, 2, 3, 4, 8, 9, 10]);
    assert_eq!(r.rooms.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [7]);
    assert_eq!(d.levels(), [(1, 0.0), (2, 3000.0)]);
    assert!((r.rooms[0].1.area - 5800.0 * 3800.0).abs() < 1e-3, "{}", r.rooms[0].1.area);
}
