use super::*;

#[test]
fn typed_values_and_sign() {
    let mut t = String::from("12.5");
    toggle_sign(&mut t);
    assert_eq!(t, "-12.5");
    toggle_sign(&mut t);
    assert_eq!(t, "12.5");
    let mut e = String::new();
    toggle_sign(&mut e);
    assert_eq!(e, "-");
    let m = |typed: &str| Modal {
        mode: Mode::Grab,
        target: Target::Sketch,
        axis: None,
        plane: false,
        typed: typed.into(),
        start: Pos2::ZERO,
        now: Pos2::ZERO,
        turned: 0.0,
        last_angle: None,
        rect: egui::Rect::ZERO,
        pivot: DVec3::ZERO,
        snap: false,
        motion: Motion::Move(DVec3::ZERO),
    };
    assert_eq!(m("-").value(), None);
    assert_eq!(m("").value(), None);
    assert_eq!(m("-3").value(), Some(-3.0));
    assert_eq!(m(".5").value(), Some(0.5));
}

#[test]
fn screen_angles_are_counter_clockwise_with_y_up() {
    let c = Pos2::new(100.0, 100.0);
    assert!(screen_angle(c, Pos2::new(110.0, 100.0)).abs() < 1e-9);
    // Above the centre on screen (smaller y) is +90 degrees.
    assert!((screen_angle(c, Pos2::new(100.0, 90.0)) - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
}

#[test]
fn sketch_motions_become_affines_about_the_pivot() {
    let pivot = DVec2::new(10.0, 0.0);
    let turn = affine_2d(pivot, Motion::Turn { axis: 2, angle: std::f64::consts::FRAC_PI_2 });
    assert!(turn.transform_point2(DVec2::new(20.0, 0.0)).distance(DVec2::new(10.0, 10.0)) < 1e-9);
    let scale = affine_2d(pivot, Motion::Scale(2.0));
    assert!(scale.transform_point2(DVec2::new(20.0, 0.0)).distance(DVec2::new(30.0, 0.0)) < 1e-9);
    let mv = affine_2d(pivot, Motion::Move(DVec3::new(1.0, 2.0, 0.0)));
    assert_eq!(mv.transform_point2(DVec2::ZERO), DVec2::new(1.0, 2.0));
}

#[test]
fn plane_hits_and_snap_steps() {
    let hit = plane_hit(DVec3::ZERO, DVec3::Z, DVec3::new(1.0, 2.0, 10.0), DVec3::NEG_Z).unwrap();
    assert!(hit.distance(DVec3::new(1.0, 2.0, 0.0)) < 1e-12);
    assert_eq!(plane_hit(DVec3::ZERO, DVec3::Z, DVec3::Z, DVec3::X), None);
    assert_eq!(snap_step_3d(150.0), 10.0);
    assert_eq!(snap_step_3d(15.0), 1.0);
}

#[test]
fn targets_limit_the_modes() {
    assert!(allowed(Mode::Scale, Target::Sketch));
    assert!(allowed(Mode::Grab, Target::Solids));
    assert!(!allowed(Mode::Rotate, Target::Solids));
    assert!(allowed(Mode::Rotate, Target::Component(1)));
    assert!(!allowed(Mode::Scale, Target::Component(1)));
}
