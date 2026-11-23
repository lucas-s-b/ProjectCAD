//! Benchmarks: fixed analytic cases built through commands and run through the code the app runs (rebuild,
//! `run_study`, `mass`, `drive`). A row within its tolerance is the only result ever labelled "validated against
//! <case>"; studies users run stay approximate. `cargo run --release -p cad-bench -- validate` writes
//! docs/VALIDATION.md from these rows.
use crate::{
    BodyOp, Command, Component, Document, Entity, FaceKey, FaceRef, Feature, FeatureKind, Frame, Joint, JointKind, Load, LoadKind,
    MassRow, MotionStudy, RebuildResult, Rebuilder, Study, StudyReport, Target,
};
use cad_analysis::{Accuracy, FOUR_BAR, TetMesh, four_bar_rocker};
use cad_geometry::{DVec2, DVec3, Shape};

/// Benchmark cases, the names `Accuracy::Validated` carries (with `cad_analysis::FOUR_BAR`).
pub const CANTILEVER: &str = "cantilever beam";
pub const TENSION_BAR: &str = "bar in tension";
pub const SOLID_BOX: &str = "solid box";
pub const SOLID_CYLINDER: &str = "solid cylinder";

/// Element sizes of the cantilever's convergence study, mm: 1, 2 and 4 elements through the 10 mm depth (2 mm would
/// need 36 300 unknowns, above `MAX_DOFS`).
pub const CANTILEVER_SIZES: [f64; 3] = [10.0, 5.0, 2.5];

/// One quantity computed by the app's own pipeline next to its analytic reference.
#[derive(Debug, Clone, PartialEq)]
pub struct Benchmark {
    /// The case; also the name in `Accuracy::Validated`.
    pub case: &'static str,
    /// What is compared, with its unit: "tip deflection (mm)".
    pub quantity: &'static str,
    /// The reference as a formula.
    pub formula: &'static str,
    pub reference: f64,
    pub result: f64,
    /// Largest accepted |`error`|: relative, or absolute in the quantity's unit when `reference` is 0.
    pub tolerance: f64,
    /// The discretisation: "h=2.5 mm: 6561 nodes, 3840 elements, 19440 unknowns", "196 triangles", "36 steps of 10°".
    pub mesh: String,
    /// The label of the computation itself (e.g. `Approximate(FEA)`, `Exact`).
    pub computed: Accuracy,
}

impl Benchmark {
    /// (result − reference) / reference; result − reference when the reference is 0.
    pub fn error(&self) -> f64 {
        if self.reference == 0.0 { self.result - self.reference } else { (self.result - self.reference) / self.reference }
    }

    /// |error| within the tolerance (false for NaN).
    pub fn passed(&self) -> bool {
        self.error().abs() <= self.tolerance
    }

    /// `Validated(case)` when passed, else the computation's own label.
    pub fn accuracy(&self) -> Accuracy {
        if self.passed() { Accuracy::Validated(self.case) } else { self.computed }
    }
}

/// Every case, in report order: cantilever (2 rows per element size), bar in tension (2), solid box (3), solid
/// cylinder (3), four-bar (1). Errors: a case that cannot be built or run (never a failed tolerance: see `passed`).
pub fn benchmarks() -> Result<Vec<Benchmark>, String> {
    let mut rows = cantilever()?;
    rows.extend(tension_bar()?);
    rows.extend(solids()?);
    rows.extend(four_bar()?);
    Ok(rows)
}

/// A document whose sketch entity 1 (`shape`, XY plane) is extruded by `depth` into body 1.
fn extruded(shape: Shape, depth: &str) -> Result<Document, String> {
    let mut d = Document::new("benchmark");
    let profile = Entity { id: 1, name: "profile".into(), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() };
    d.apply(Command::Add(profile)).map_err(|err| err.to_string())?;
    let kind = FeatureKind::Extrude { profile: vec![1], distance: depth.into(), offset: "0".into(), taper: "0 deg".into(), op: BodyOp::New, flip: false, target: Target::Auto };
    d.apply(Command::AddFeature { index: 0, feature: Feature { id: 1, name: "body".into(), suppressed: false, kind } }).map_err(|err| err.to_string())?;
    Ok(d)
}

/// The planar face of body 1 whose outward normal is `n`.
fn plane_face(r: &RebuildResult, n: DVec3) -> Result<FaceRef, String> {
    let body = &r.bodies.first().ok_or("the benchmark body did not build")?.1;
    (0..body.faces.len())
        .find(|&i| FaceKey::of(&body.faces[i]).matches(&FaceKey::Plane(n)))
        .and_then(|i| FaceRef::of(body, i))
        .ok_or_else(|| format!("no face along {n}"))
}

/// The node of `mesh` at `p` (box meshes have a node on every half-grid point).
fn node_at(mesh: &TetMesh, p: DVec3) -> Result<usize, String> {
    mesh.nodes.iter().position(|q| q.distance(p) <= 1e-9).ok_or_else(|| format!("no mesh node at {p}"))
}

fn mesh_text(rep: &StudyReport) -> String {
    format!("h={} mm: {} nodes, {} elements, {} unknowns", rep.h, rep.stats.nodes, rep.stats.elements, rep.result.dofs)
}

/// The box 100 x 10 x 10 mm from the origin (body 1) with one steel study (`Study::new`, ν replaced by `poisson`) at
/// element size `h` mm: the −X end fixed, `force` N spread over the +X end; and the rebuild the study reads.
/// `cad-bench` times it and writes it as a project file.
pub fn beam(h: f64, poisson: &str, force: [&str; 3]) -> Result<(Document, RebuildResult), String> {
    let mut d = extruded(Shape::rect(DVec2::ZERO, DVec2::new(100.0, 10.0)), "10 mm")?;
    let r = Rebuilder::default().rebuild(&d);
    let study = Study {
        poisson: poisson.into(),
        fixed: vec![plane_face(&r, -DVec3::X)?],
        loads: vec![Load { face: plane_face(&r, DVec3::X)?, kind: LoadKind::Force(force.map(String::from)) }],
        ..Study::new("benchmark".into(), 1, format!("{h} mm"))
    };
    d.apply(Command::SetStudies(vec![study])).map_err(|err| err.to_string())?;
    Ok((d, r))
}

/// Study 0 of `beam(h, poisson, force)`.
fn beam_study(h: f64, poisson: &str, force: [&str; 3]) -> Result<StudyReport, String> {
    let (d, r) = beam(h, poisson, force)?;
    d.run_study(&r, 0)
}

/// Steel cantilever L = 100, b = 10, depth 10 mm (E 210 000 MPa, ν 0.3), clamped at x = 0, F = 100 N along −Z over the
/// free end, at each of `CANTILEVER_SIZES`: −u_z of the end's centre node (100, 5, 5) against Timoshenko, and the von
/// Mises stress at the top node of mid-span (50, 5, 10) against the bending stress (the clamped root holds a corner
/// singularity, so it is not compared).
fn cantilever() -> Result<Vec<Benchmark>, String> {
    let (f, l, b, depth, young, nu): (f64, f64, f64, f64, f64, f64) = (100.0, 100.0, 10.0, 10.0, 210_000.0, 0.3);
    let (inertia, area, shear) = (b * depth.powi(3) / 12.0, b * depth, young / (2.0 * (1.0 + nu)));
    let deflection = f * l.powi(3) / (3.0 * young * inertia) + f * l / (5.0 / 6.0 * shear * area);
    let stress = f * (l - 50.0) * (depth / 2.0) / inertia;
    let mut rows = Vec::new();
    for h in CANTILEVER_SIZES {
        let rep = beam_study(h, "0.3", ["0", "0", "-100"])?;
        let tip = -rep.result.displacement[node_at(&rep.mesh, DVec3::new(100.0, 5.0, 5.0))?].z;
        let mid = rep.result.von_mises[node_at(&rep.mesh, DVec3::new(50.0, 5.0, 10.0))?];
        let mesh = mesh_text(&rep);
        rows.push(Benchmark { case: CANTILEVER, quantity: "tip deflection (mm)", formula: "Timoshenko: F L³ / (3 E I) + F L / (κ G A), κ = 5/6, I = b h³ / 12", reference: deflection, result: tip, tolerance: 0.03, mesh: mesh.clone(), computed: rep.accuracy });
        rows.push(Benchmark { case: CANTILEVER, quantity: "bending stress at mid-span, top (MPa)", formula: "σ = M c / I = F (L − x) (h / 2) / I, x = L / 2", reference: stress, result: mid, tolerance: 0.03, mesh, computed: rep.accuracy });
    }
    Ok(rows)
}

/// The same box with ν = 0, 1000 N along +X at h 5 mm: the exact displacement u = F x / (E A) is linear, which
/// quadratic tetrahedra reproduce to round-off.
fn tension_bar() -> Result<Vec<Benchmark>, String> {
    let rep = beam_study(5.0, "0", ["1000", "0", "0"])?;
    let (f, l, young, area) = (1000.0, 100.0, 210_000.0, 100.0);
    let mesh = mesh_text(&rep);
    Ok(vec![
        Benchmark { case: TENSION_BAR, quantity: "end displacement (mm)", formula: "δ = F L / (E A)", reference: f * l / (young * area), result: rep.result.max_displacement().0, tolerance: 1e-9, mesh: mesh.clone(), computed: rep.accuracy },
        Benchmark { case: TENSION_BAR, quantity: "axial stress (MPa)", formula: "σ = F / A", reference: f / area, result: rep.result.max_von_mises().0, tolerance: 1e-9, mesh, computed: rep.accuracy },
    ])
}

/// Mass properties of body 1 of `d` at the assumed 1 g/cm³, and its triangle count.
fn body_mass(d: &Document) -> Result<(MassRow, usize), String> {
    let r = Rebuilder::default().rebuild(d);
    let triangles = r.bodies.first().ok_or("the benchmark body did not build")?.1.mesh.triangle_count();
    let (_, row) = d.mass(&r).bodies.into_iter().next().ok_or("no mass properties")?;
    Ok((row, triangles))
}

/// Mass properties at 1 g/cm³, inertia about the centre of mass (g·mm²): the box a = 100, b = 10, c = 10 mm (planar
/// faces: exact) and the cylinder r = 10 mm, 20 mm long (integrated on its facets, so the tolerance covers the facet
/// error).
fn solids() -> Result<Vec<Benchmark>, String> {
    let (bx, triangles) = body_mass(&extruded(Shape::rect(DVec2::ZERO, DVec2::new(100.0, 10.0)), "10 mm")?)?;
    let (a, b, c) = (100.0, 10.0, 10.0);
    let m = a * b * c / 1000.0;
    let mesh = format!("{triangles} triangles");
    let boxed = |quantity, formula, reference, result| Benchmark { case: SOLID_BOX, quantity, formula, reference, result, tolerance: 1e-9, mesh: mesh.clone(), computed: bx.accuracy };
    let mut rows = vec![
        boxed("volume (mm³)", "V = a b c", a * b * c, bx.props.volume),
        boxed("Ixx about the centre of mass (g·mm²)", "m (b² + c²) / 12", m * (b * b + c * c) / 12.0, bx.props.inertia.x_axis.x),
        boxed("Iyy about the centre of mass (g·mm²)", "m (a² + c²) / 12", m * (a * a + c * c) / 12.0, bx.props.inertia.y_axis.y),
    ];
    let (cy, triangles) = body_mass(&extruded(Shape::Circle { center: DVec2::ZERO, radius: 10.0 }, "20 mm")?)?;
    let (radius, len) = (10.0, 20.0);
    let volume = std::f64::consts::PI * radius * radius * len;
    let m = volume / 1000.0;
    let mesh = format!("{triangles} triangles");
    let round = |quantity, formula, reference, result, tolerance| Benchmark { case: SOLID_CYLINDER, quantity, formula, reference, result, tolerance, mesh: mesh.clone(), computed: cy.accuracy };
    rows.push(round("volume (mm³)", "V = π r² L", volume, cy.props.volume, 0.005));
    rows.push(round("Ixx about the centre of mass (g·mm²)", "m (3 r² + L²) / 12", m * (3.0 * radius * radius + len * len) / 12.0, cy.props.inertia.x_axis.x, 0.01));
    rows.push(round("Izz, the axis (g·mm²)", "m r² / 2", m * radius * radius / 2.0, cy.props.inertia.z_axis.z, 0.01));
    Ok(rows)
}

/// The Grashof crank-rocker of the loop tests (ground pivots (0, 0) and (50, 0), crank a = 20, coupler b = 60, rocker
/// c = 40 mm) driven through 360° in 36 steps: the largest difference, in degrees, between the solved rocker angle and
/// Freudenstein's (`four_bar_rocker`, branch 1).
fn four_bar() -> Result<Vec<Benchmark>, String> {
    let mut d = extruded(Shape::rect(DVec2::ZERO, DVec2::new(10.0, 20.0)), "5 mm")?;
    let s = |v: [&str; 3]| v.map(String::from);
    let (o, z) = (["0", "0", "0"], ["0", "0", "1"]);
    let mut cmds = Vec::new();
    for (id, grounded) in [(1, true), (2, false), (3, false), (4, false)] {
        cmds.push(Command::AddComponent(Component { id, name: format!("c{id}"), body: 1, at: s(o), angles: s(o), meta: Default::default(), grounded }));
    }
    let joint = |id, parent, child, origin: [&str; 3], value: &str, free: bool| Joint {
        id,
        kind: JointKind::Revolute,
        parent,
        child,
        parent_frame: Frame { origin: s(origin), axis: s(z), face: None },
        child_frame: Frame::from_vectors(DVec3::ZERO, -DVec3::Z),
        value: value.into(),
        min: None,
        max: None,
        closes_loop: false,
        free,
    };
    cmds.push(Command::AddJoint(joint(1, 1, 2, o, "0 deg", false)));
    cmds.push(Command::AddJoint(joint(2, 2, 3, ["20 mm", "0", "0"], "40 deg", true)));
    cmds.push(Command::AddJoint(joint(3, 1, 4, ["50 mm", "0", "0"], "60 deg", true)));
    let close = Joint { child_frame: Frame::from_vectors(DVec3::new(40.0, 0.0, 0.0), -DVec3::Z), closes_loop: true, ..joint(4, 3, 4, ["60 mm", "0", "0"], "0", false) };
    cmds.push(Command::AddJoint(close));
    for cmd in cmds {
        d.apply(cmd).map_err(|err| err.to_string())?;
    }
    let r = Rebuilder::default().rebuild(&d);
    let study = MotionStudy { joint: 1, start: "0".into(), end: "360 deg".into(), steps: 36, trace: 4, point: s(["40 mm", "0", "0"]) };
    let m = d.drive(&r, &study)?;
    let mut worst = 0.0f64;
    for (&theta2, p) in m.values.iter().zip(&m.trace) {
        let want = four_bar_rocker(20.0, 60.0, 40.0, 50.0, theta2, 1.0).ok_or("the four-bar does not assemble")?;
        let diff = p.y.atan2(p.x - 50.0) - want;
        worst = worst.max(diff.sin().atan2(diff.cos()).abs().to_degrees());
    }
    Ok(vec![Benchmark {
        case: FOUR_BAR,
        quantity: "rocker angle, largest error over 37 positions (deg)",
        formula: "Freudenstein position equation, a = 20, b = 60, c = 40, d = 50 mm",
        reference: 0.0,
        result: worst,
        tolerance: 1e-6,
        mesh: "36 steps of 10°".into(),
        computed: m.accuracy,
    }])
}

const INTRO: &str = "Each case builds its model through the app's own commands (sketch, extrude, components and joints) and runs the code the app runs (`Document::run_study`, `Document::mass`, `Document::drive`), then compares one number with an analytic reference. Error = (result − reference) / reference; for the four-bar, whose reference is 0, it is the angle difference in degrees. A row within its tolerance is labelled \"validated against <case>\". Studies run in the app are not these cases and keep their own label, \"approximate (FEA, linear elastic, mesh h=…)\"; the Model tab's \"Validation report\" section runs the same rows on demand.";

const NOTES: &str = "Notes:
- Cantilever beam: steel, E = 210 000 MPa, ν = 0.3, L = 100 mm, b = h = 10 mm, F = 100 N spread over the free end, the other end fully clamped. Timoshenko gives 0.191962 mm (Euler-Bernoulli alone 0.190476 mm). The deflection error falls with the element size; what remains at h = 2.5 mm is mostly the fully clamped 3D root, which also holds the section's Poisson contraction and is stiffer than the beam theory's clamp. The stress is read at mid-span: at the clamped root the corners are a singularity whose peak grows with refinement.
- Bar in tension: ν = 0, so the exact displacement is linear in x and the quadratic elements reproduce it to round-off.
- Solid cylinder: the volume and inertia of the faceted mesh, not of the true cylinder, hence a tolerance above the facet error.
- Four-bar: Grashof crank-rocker, loops closed by Levenberg-Marquardt to 1e-9 mm at every step.
- Not covered yet: thick-walled cylinder under pressure (Lamé; needs roller supports and curved elements), gravity, pressure on curved faces, stress concentrations.
";

/// docs/VALIDATION.md: title, `header` (date, profile, totals), how to read it, one table row per benchmark, notes.
pub fn validation_markdown(rows: &[Benchmark], header: &str) -> String {
    let mut out = format!(
        "# Validation report\n\nGenerated by `cargo run --release -p cad-bench -- validate`; do not edit by hand.\n\n{header}\n\n{INTRO}\n\n| Case | Quantity | Reference formula | Reference | Result | Error | Tolerance | Pass | Mesh | Label |\n|---|---|---|---|---|---|---|---|---|---|\n"
    );
    for b in rows {
        let pass = if b.passed() { "yes" } else { "NO" };
        out += &format!(
            "| {} | {} | {} | {:.9} | {:.9} | {:+.2e} | {:.0e} | {pass} | {} | {} |\n",
            b.case, b.quantity, b.formula, b.reference, b.result, b.error(), b.tolerance, b.mesh, b.accuracy()
        );
    }
    out + "\n" + NOTES
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
