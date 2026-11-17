//! Benchmark baseline: `cargo run --release -p cad-bench` prints a Markdown table.
use cad_constraints::{Constraint, PointKind, PointRef, solve};
use cad_document::{BodyOp, Command, Document, Entity, Feature, FeatureKind, History, Rebuilder, Target};
use cad_geometry::snap::{intersections, object_snap};
use cad_geometry::{DVec2, DVec3, Shape, distance_2d, tessellate};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

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
}

fn main() {
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

    let cpus = std::thread::available_parallelism().map_or(1, usize::from);
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    println!("Date: <date> | profile: {profile} | CPUs: {cpus} | file size of the 100 000-line project: {:.1} MB\n", size as f64 / 1e6);
    println!("| Case | N | Median ms |\n|---|---|---|");
    for (case, n, ms) in &t.0 {
        println!("| {case} | {n} | {ms:.3} |");
    }
}
