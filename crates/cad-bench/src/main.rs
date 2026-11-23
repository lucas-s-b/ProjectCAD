//! Benchmark baseline: `cargo run --release -p cad-bench` prints a Markdown table of timings; `cargo run --release -p cad-bench -- validate` runs the analytic validation cases and writes docs/VALIDATION.md; `cargo run -p cad-bench -- assembly <path> [n] [pitch]` writes the benchmark assembly (n components of one gear, default 200, `pitch` mm apart, default 100) as a project file; `-- pin <path>` and `-- beam <path>` write the motion and static-study models of the analysis cases.
use cad_analysis::{Elastic, element_stiffness, internal_forces, nodal_von_mises};
use cad_constraints::{Constraint, PointKind, PointRef, solve};
use cad_document::{BodyOp, Bvh, Command, Component, Document, Entity, Feature, FeatureKind, Frame, History, Joint, JointKind, MotionStudy, PartDef, Rebuilder, Target};
use cad_geometry::snap::{intersections, object_snap};
use cad_geometry::{DVec2, DVec3, Shape, distance_2d, tessellate};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const SAMPLES: usize = 7;
/// Side of the square the random lines live in, in millimetres (10 m).
const WORLD: f64 = 10_000.0;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Uniform in [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Short lines (up to 20 mm long) at random places.
fn random_lines(n: usize) -> Vec<Shape> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    (0..n)
        .map(|_| {
            let a = DVec2::new(rng.unit() * WORLD, rng.unit() * WORLD);
            let d = DVec2::new(rng.unit() * 20.0 - 10.0, rng.unit() * 20.0 - 10.0) + DVec2::new(0.5, 0.5);
            Shape::Line { a, b: a + d }
        })
        .collect()
}

fn document(shapes: &[Shape]) -> Document {
    let mut doc = Document::new("bench");
    for s in shapes {
        let id = doc.alloc_id();
        doc.apply(Command::Add(Entity { id, name: String::new(), shape: s.clone(), layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).expect("valid line");
    }
    doc
}

/// Median of `SAMPLES` runs after one warm-up run, in milliseconds.
fn median_ms<R>(mut f: impl FnMut() -> R) -> f64 {
    black_box(f());
    let mut t: Vec<f64> = (0..SAMPLES)
        .map(|_| {
            let start = Instant::now();
            black_box(f());
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    t.sort_by(f64::total_cmp);
    t[SAMPLES / 2]
}

struct Table(Vec<(String, String, f64)>);

impl Table {
    fn add<R>(&mut self, case: &str, n: impl ToString, f: impl FnMut() -> R) {
        let ms = median_ms(f);
        eprintln!("{case} {}: {ms:.3} ms", n.to_string());
        self.0.push((case.to_string(), n.to_string(), ms));
    }
}

/// A chain of N lines joined end to end, alternately horizontal and vertical, each 10 mm long.
fn chain(n: usize) -> (BTreeMap<u64, Shape>, Vec<Constraint>) {
    let mut rng = Rng(42);
    let pt = |entity: u64, kind| PointRef { entity, kind };
    let mut shapes = BTreeMap::new();
    let mut cs = vec![Constraint::Fixed { point: pt(1, PointKind::Start), at: DVec2::ZERO }];
    let mut at = DVec2::ZERO;
    for i in 0..n as u64 {
        let id = i + 1;
        let dir = if i % 2 == 0 { DVec2::X } else { DVec2::Y };
        let jitter = DVec2::new(rng.unit() - 0.5, rng.unit() - 0.5) * 2.0;
        let b = at + dir * 12.0 + jitter;
        shapes.insert(id, Shape::Line { a: at, b });
        cs.push(if i % 2 == 0 { Constraint::Horizontal(id) } else { Constraint::Vertical(id) });
        cs.push(Constraint::Distance(pt(id, PointKind::Start), pt(id, PointKind::End), 10.0));
        if i > 0 {
            cs.push(Constraint::Coincident(pt(id - 1, PointKind::End), pt(id, PointKind::Start)));
        }
        at = b;
    }
    (shapes, cs)
}

/// A 60 x 50 mm plate with 20 holes of radius 2 mm (features 2 to 21 each cut one hole); returns the id of the last hole.
fn plate_with_holes(doc: &mut Document) -> u64 {
    let add = |doc: &mut Document, shape: Shape| {
        let id = doc.alloc_id();
        doc.apply(Command::Add(Entity { id, name: String::new(), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).expect("valid shape");
        id
    };
    let plate = add(doc, Shape::rect(DVec2::ZERO, DVec2::new(60.0, 50.0)));
    let mut features = vec![(plate, BodyOp::Add)];
    for i in 0..20 {
        let center = DVec2::new(10.0 + 10.0 * (i % 5) as f64, 10.0 + 10.0 * (i / 5) as f64);
        features.push((add(doc, Shape::Circle { center, radius: 2.0 }), BodyOp::Cut));
    }
    for (i, (profile, op)) in features.iter().enumerate() {
        let kind = FeatureKind::Extrude { profile: vec![*profile], distance: "5".into(), offset: "0".into(), taper: "0 deg".into(), op: *op, flip: false, target: Target::Auto };
        let feature = Feature { id: doc.alloc_feature_id(), name: format!("f{i}"), suppressed: false, kind };
        doc.apply(Command::AddFeature { index: i, feature }).expect("valid feature");
    }
    features[20].0
}

fn plate_cases(t: &mut Table) {
    let mut doc = Document::new("plate");
    let last_hole = plate_with_holes(&mut doc);
    let cold = Rebuilder::default().rebuild(&doc);
    assert!(cold.bodies.len() == 1 && cold.recomputed == 21, "the plate must build, otherwise the timing means nothing");
    t.add("rebuild plate with 20 cut holes (cold)", format!("21 features, {} recomputed", cold.recomputed), || Rebuilder::default().rebuild(&doc).recomputed);
    let mut rebuilder = Rebuilder::default();
    rebuilder.rebuild(&doc);
    let (mut flip, mut recomputed) = (false, 0);
    let mut change_last_hole = || {
        flip = !flip;
        let radius = if flip { 2.5 } else { 2.0 };
        doc.apply(Command::SetShape { id: last_hole, shape: Shape::Circle { center: DVec2::new(50.0, 40.0), radius } }).expect("valid circle");
        recomputed = rebuilder.rebuild(&doc).recomputed;
        recomputed
    };
    let ms = median_ms(&mut change_last_hole);
    assert_eq!(recomputed, 1);
    eprintln!("incremental: {ms:.3} ms");
    t.0.push(("the same after changing one hole radius (incremental)".to_string(), format!("21 features, {recomputed} recomputed (last hole)"), ms));
}

/// A 110 x 110 plate with one hole and two linear patterns of 10 (a 10 x 10 grid, 100 holes).
fn pattern_case(t: &mut Table) {
    let mut doc = Document::new("pattern");
    let add = |doc: &mut Document, shape: Shape| {
        let id = doc.alloc_id();
        doc.apply(Command::Add(Entity { id, name: String::new(), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).expect("valid shape");
        id
    };
    let plate = add(&mut doc, Shape::rect(DVec2::ZERO, DVec2::splat(110.0)));
    let hole = add(&mut doc, Shape::Circle { center: DVec2::splat(10.0), radius: 2.0 });
    let extrude = |profile, op| FeatureKind::Extrude { profile: vec![profile], distance: "5".into(), offset: "0".into(), taper: "0 deg".into(), op, flip: false, target: Target::Auto };
    let linear = |source, dx: &str, dy: &str| FeatureKind::LinearPattern { source, count: "10".into(), dx: dx.into(), dy: dy.into(), dz: "0".into() };
    let kinds = [extrude(plate, BodyOp::Add), extrude(hole, BodyOp::Cut), linear(2, "10", "0"), linear(3, "0", "10")];
    for (i, kind) in kinds.into_iter().enumerate() {
        let feature = Feature { id: doc.alloc_feature_id(), name: format!("f{i}"), suppressed: false, kind };
        doc.apply(Command::AddFeature { index: i, feature }).expect("valid feature");
    }
    let cold = Rebuilder::default().rebuild(&doc);
    assert!(cold.bodies.len() == 1 && cold.recomputed == 4, "the plate must build, otherwise the timing means nothing");
    let triangles = cold.bodies[0].1.mesh.triangle_count();
    t.add("rebuild plate with a 10 x 10 hole pattern (cold)", format!("4 features, {triangles} triangles"), || Rebuilder::default().rebuild(&doc).recomputed);
    let path = std::env::temp_dir().join("cad-bench-pattern.cadproj");
    doc.save(&path).expect("save the pattern plate");
    t.add("cold open: load + rebuild of the 10 x 10 pattern plate", "4 features", || Rebuilder::default().rebuild(&Document::load(&path).expect("load")).recomputed);
}

/// `n` components of one gear (body 1: module 2 mm, 40 teeth, 10 mm wide, 8 mm bore) on the XY plane, ten per row,
/// `pitch` mm apart, named "Gear #<id>". The tip diameter is 84 mm, so neighbours closer than that interfere.
fn assembly_at(n: usize, pitch: f64) -> Document {
    let mut doc = Document::new("assembly");
    let def = PartDef::Gear { module: "2 mm".into(), teeth: "40".into(), pressure: "20 deg".into(), width: "10 mm".into(), bore: "8 mm".into() };
    let kind = FeatureKind::Part { def, sketch: 0, x: "0".into(), y: "0".into(), op: BodyOp::New, target: Target::Auto };
    let body = doc.alloc_feature_id();
    doc.apply(Command::AddFeature { index: 0, feature: Feature { id: body, name: "Gear".into(), suppressed: false, kind } }).expect("valid gear");
    for i in 0..n {
        let id = doc.alloc_component_id();
        let at = [format!("{} mm", (i % 10) as f64 * pitch), format!("{} mm", (i / 10) as f64 * pitch), "0".into()];
        let c = Component { id, name: format!("Gear #{id}"), body, at, angles: ["0".into(), "0".into(), "0".into()], meta: Default::default(), grounded: false };
        doc.apply(Command::AddComponent(c)).expect("valid component");
    }
    doc
}

/// `assembly_at(n, 100.0)`: neighbours 16 mm apart.
fn assembly(n: usize) -> Document {
    assembly_at(n, 100.0)
}

/// Body 1: a 40 x 40 x 5 mm plate with a hole of 10 mm diameter at (20, 20); body 3: a pin of 8 mm diameter, 20 mm
/// long. Component 1 (the plate, grounded) and component 2 (the pin) are joined by revolute joint 1 on the hole's
/// axis, 2.5 mm up, so the pin passes through the hole with 1 mm clearance and never touches the plate.
fn pin() -> Document {
    let mut doc = Document::new("pin");
    let add = |doc: &mut Document, shape: Shape| {
        let id = doc.alloc_id();
        doc.apply(Command::Add(Entity { id, name: String::new(), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).expect("valid shape");
        id
    };
    let plate = add(&mut doc, Shape::rect(DVec2::ZERO, DVec2::splat(40.0)));
    let hole = add(&mut doc, Shape::Circle { center: DVec2::splat(20.0), radius: 5.0 });
    let pin = add(&mut doc, Shape::Circle { center: DVec2::ZERO, radius: 4.0 });
    let extrude = |profile, distance: &str, op| FeatureKind::Extrude { profile: vec![profile], distance: distance.into(), offset: "0".into(), taper: "0 deg".into(), op, flip: false, target: Target::Auto };
    let kinds = [extrude(plate, "5", BodyOp::New), extrude(hole, "5", BodyOp::Cut), extrude(pin, "20", BodyOp::New)];
    for (i, kind) in kinds.into_iter().enumerate() {
        let feature = Feature { id: doc.alloc_feature_id(), name: format!("f{i}"), suppressed: false, kind };
        doc.apply(Command::AddFeature { index: i, feature }).expect("valid feature");
    }
    let s = |v: [&str; 3]| v.map(String::from);
    let comp = |id, body, grounded| Component { id, name: format!("c{id}"), body, at: s(["0", "0", "0"]), angles: s(["0", "0", "0"]), meta: Default::default(), grounded };
    doc.apply(Command::AddComponent(comp(1, 1, true))).expect("valid component");
    doc.apply(Command::AddComponent(comp(2, 3, false))).expect("valid component");
    let joint = Joint {
        id: 1,
        kind: JointKind::Revolute,
        parent: 1,
        child: 2,
        parent_frame: Frame { origin: s(["20 mm", "20 mm", "2.5 mm"]), axis: s(["0", "0", "1"]), face: None },
        child_frame: Frame::from_vectors(DVec3::ZERO, -DVec3::Z),
        value: "0 deg".into(),
        min: None,
        max: None,
        closes_loop: false,
        free: false,
    };
    doc.apply(Command::AddJoint(joint)).expect("valid joint");
    doc
}

/// One turn of the pin of `pin()` in `steps` steps, tracing a point on its rim.
fn pin_turn(steps: usize) -> MotionStudy {
    let s = |v: [&str; 3]| v.map(String::from);
    MotionStudy { joint: 1, start: "0".into(), end: "360 deg".into(), steps, trace: 2, point: s(["4 mm", "0", "0"]) }
}

/// The analyses that run on the UI thread when their button is clicked: a static study near `MAX_DOFS`, its mesh and
/// its element loops, the validation benchmarks, an interference check and a motion study with its collision check.
fn analysis_cases(t: &mut Table) {
    let (doc, r) = cad_document::validation::beam(2.45, "0.3", ["0", "0", "-100"]).expect("the beam builds");
    let rep = doc.run_study(&r, 0).expect("the beam study solves");
    let dofs = rep.result.dofs;
    assert!(dofs >= 25_000, "a study near MAX_DOFS, otherwise the timing means nothing: {dofs}");
    t.add("static study: steel cantilever 100 x 10 x 10 mm, h 2.45 mm", format!("{dofs} unknowns"), || doc.run_study(&r, 0).map(|s| s.result.dofs));
    t.add("analysis mesh of the same beam (Mesh for analysis)", format!("{} elements", rep.stats.elements), || doc.mesh_body(&r, 1, "2.45 mm").map(|m| m.stats.elements));
    let mat = Elastic { young: 210_000.0, poisson: 0.3 };
    let (mesh, u) = (&rep.mesh, &rep.result.displacement);
    t.add("element loops of that study: stiffness, internal forces, von Mises", rep.stats.elements, || {
        let mut sum = 0.0;
        for el in &mesh.elements {
            sum += black_box(element_stiffness(&el.map(|v| mesh.nodes[v as usize]), mat))[0][0].x_axis.x;
        }
        sum + internal_forces(mesh, mat, u)[0].x + nodal_von_mises(mesh, mat, u)[0]
    });
    t.add("validation benchmarks (Run benchmarks)", "15 cases", || cad_document::benchmarks().map(|rows| rows.len()));
    let gears = assembly_at(20, 80.0);
    let gr = Rebuilder::default().rebuild(&gears);
    let meetings = gears.check(&gr, "5 mm").expect("a valid limit").meetings.len();
    assert!(meetings > 0, "neighbours 80 mm apart must meet, otherwise the timing means nothing");
    t.add("interference check: 20 gears 80 mm apart", format!("{meetings} meetings"), || gears.check(&gr, "5 mm").map(|c| c.meetings.len()));
    let p = pin();
    let pr = Rebuilder::default().rebuild(&p);
    let ((a0, a1), (b0, b1)) = (pr.component(1).expect("plate placed").bbox(), pr.component(2).expect("pin placed").bbox());
    assert!(a0.cmple(b1).all() && b0.cmple(a1).all(), "the pin must sit in the plate's box, otherwise every sample skips the boolean");
    let study = pin_turn(360);
    let motion = p.drive(&pr, &study).expect("the pin turns");
    assert!(matches!(p.first_collision(&pr, &motion), Ok(None)), "the pin must turn freely, otherwise the check stops at the first sample");
    t.add("motion study with collision check: a pin turning in a plate hole", "360 steps", || p.drive(&pr, &study).and_then(|m| p.first_collision(&pr, &m)).map(|c| c.is_some()));
}

/// 200 components of one gear: a cold rebuild, a rebuild after a change that moves nothing (warm), the feature edges
/// every 3D scene build recomputes per component, and the assembly's mass properties (read when a rebuild arrives).
fn assembly_cases(t: &mut Table) {
    let mut doc = assembly(200);
    let cold = Rebuilder::default().rebuild(&doc);
    assert!(cold.components.len() == 200 && cold.components.iter().all(|(_, r)| r.is_ok()), "the assembly must place, otherwise the timing means nothing");
    let triangles = cold.bodies[0].1.mesh.triangle_count();
    t.add("assembly rebuild: a gear and 200 components of it (cold)", format!("200 x {triangles} triangles"), || Rebuilder::default().rebuild(&doc).components.len());
    let mut rebuilder = Rebuilder::default();
    rebuilder.rebuild(&doc);
    let (mut flip, mut recomputed) = (false, usize::MAX);
    let ms = median_ms(|| {
        flip = !flip;
        doc.apply(Command::SetParam { name: "unused".into(), expr: Some(if flip { "1" } else { "2" }.into()) }).expect("valid parameter");
        recomputed = rebuilder.rebuild(&doc).recomputed;
        recomputed
    });
    assert_eq!(recomputed, 0);
    eprintln!("assembly warm: {ms:.3} ms");
    t.0.push(("the same after changing an unused parameter (warm)".to_string(), "200 components, 0 features recomputed".to_string(), ms));
    t.add("feature edges of every component (each 3D scene build)", 200, || cold.components.iter().map(|(_, r)| r.as_ref().map_or(0, |b| b.feature_edges().len())).sum::<usize>());
    t.add("mass properties of the assembly", 200, || doc.mass(&cold).components.len());
    t.add("BVH of every component (first 3D click in an assembly)", 200, || cold.components.iter().filter_map(|(_, r)| r.as_ref().ok()).map(|b| {
        black_box(Bvh::build(&b.mesh));
        1
    }).sum::<usize>());
    t.add("bill of materials of the assembly (each frame of the Assembly tab)", 200, || doc.bom(&cold).len());
}

/// "2026-11-23": the UTC date `secs` seconds after 1970-01-01 (Howard Hinnant's days-to-civil algorithm).
fn utc_date(secs: u64) -> String {
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Runs `cad_document::benchmarks`, writes docs/VALIDATION.md, prints it, and exits with 1 when a case fails its
/// tolerance or cannot run.
fn validate() {
    let start = Instant::now();
    let rows = cad_document::benchmarks().unwrap_or_else(|err| {
        eprintln!("a benchmark could not run: {err}");
        std::process::exit(1)
    });
    let secs = start.elapsed().as_secs_f64();
    let passed = rows.iter().filter(|b| b.passed()).count();
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let header = format!("Date: {} (UTC) | profile: {profile} | {passed} of {} cases within tolerance | {secs:.2} s", utc_date(now), rows.len());
    let text = cad_document::validation_markdown(&rows, &header);
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/VALIDATION.md");
    std::fs::write(path, &text).expect("write docs/VALIDATION.md");
    print!("{text}");
    if passed < rows.len() {
        std::process::exit(1);
    }
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("validate") {
        validate();
        return;
    }
    let arg = |i: usize| std::env::args().nth(i);
    if arg(1).as_deref() == Some("assembly") {
        let path = arg(2).expect("usage: cad-bench assembly <path> [components] [pitch mm]");
        let n = arg(3).map_or(200, |s| s.parse().expect("a component count"));
        let pitch = arg(4).map_or(100.0, |s| s.parse().expect("a pitch in mm"));
        assembly_at(n, pitch).save(std::path::Path::new(&path)).expect("save the assembly");
        return;
    }
    if arg(1).as_deref() == Some("pin") {
        pin().save(std::path::Path::new(&arg(2).expect("usage: cad-bench pin <path>"))).expect("save the pin");
        return;
    }
    if arg(1).as_deref() == Some("beam") {
        let (beam, _) = cad_document::validation::beam(2.45, "0.3", ["0", "0", "-100"]).expect("the beam builds");
        beam.save(std::path::Path::new(&arg(2).expect("usage: cad-bench beam <path>"))).expect("save the beam");
        return;
    }
    let mut t = Table(Vec::new());
    let cursor = DVec2::splat(WORLD / 2.0);
    for n in [10_000, 100_000] {
        let lines = random_lines(n);
        t.add("object_snap", n, || object_snap(cursor, 10.0, None, lines.iter()));
    }
    let lines = random_lines(100_000);
    t.add("nearest-entity pick (distance_2d)", 100_000, || {
        lines.iter().map(|s| distance_2d(s, cursor)).fold(f64::INFINITY, f64::min)
    });
    let few = random_lines(300);
    t.add("intersections, all pairs", 300, || {
        let mut hits = 0;
        for (i, a) in few.iter().enumerate() {
            for b in &few[i + 1..] {
                hits += intersections(a, b).len();
            }
        }
        hits
    });
    for n in [10, 50, 150, 400] {
        let (shapes, cs) = chain(n);
        solve(&shapes, &cs).expect("the chain must converge, otherwise the timing means nothing");
        t.add("solver, line chain", n, || solve(&shapes, &cs).map(|s| s.iterations));
    }
    let adds: Vec<Shape> = random_lines(10_000);
    t.add("history: 10000 executes + 10000 undos", 10_000, || {
        let (mut doc, mut history) = (Document::new("h"), History::default());
        for s in &adds {
            let id = doc.alloc_id();
            history.execute(&mut doc, vec![Command::Add(Entity { id, name: String::new(), shape: s.clone(), layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })]).expect("add");
        }
        while history.undo(&mut doc) {}
        doc.len()
    });
    let doc = document(&lines);
    let path = std::env::temp_dir().join(format!("cad_bench_{}.cadproj", std::process::id()));
    t.add("save", 100_000, || doc.save(&path).expect("save"));
    let size = std::fs::metadata(&path).map_or(0, |m| m.len());
    t.add("load", 100_000, || Document::load(&path).expect("load").len());
    let _ = std::fs::remove_file(&path);
    t.add("to_dxf", 100_000, || cad_import_export::to_dxf(&doc, 0).len());
    t.add("to_svg", 100_000, || cad_import_export::to_svg(&doc, 0).len());
    let dxf = cad_import_export::to_dxf(&doc, 0);
    t.add("from_dxf", 100_000, || cad_import_export::from_dxf(&dxf, "b").map(|(d, _)| d.len()));
    let sphere = Shape::Sphere { center: DVec3::ZERO, radius: 10.0 };
    t.add("tessellate spheres", 2_000, || (0..2_000).map(|_| tessellate(&sphere).map_or(0, |m| m.positions.len())).sum::<usize>());
    let square = [DVec2::ZERO, DVec2::new(1000.0, 0.0), DVec2::splat(1000.0), DVec2::new(0.0, 1000.0)];
    t.add("hatch_lines, 1 m square, spacing 0.5 mm", 1, || cad_geometry::annot::hatch_lines(&square, 0.7, 0.5).len());

    plate_cases(&mut t);
    pattern_case(&mut t);
    assembly_cases(&mut t);
    analysis_cases(&mut t);

    let cpus = std::thread::available_parallelism().map_or(1, usize::from);
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    println!("Date: <date> | profile: {profile} | CPUs: {cpus} | file size of the 100 000-line project: {:.1} MB\n", size as f64 / 1e6);
    println!("| Case | N | Median ms |\n|---|---|---|");
    for (case, n, ms) in &t.0 {
        println!("| {case} | {n} | {ms:.3} |");
    }
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
