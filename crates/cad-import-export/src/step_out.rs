//! STEP (ISO 10303-21, AP214) export of solid bodies, in millimetres: analytic B-Reps where possible, faceted otherwise.
use cad_geometry::DVec3;
use cad_modeler::{Body, EdgeGeom, Surface, TrimmedBody};
use std::fmt::Write;

/// Entity writer: `add` writes `#n=<text>;` and returns n.
struct W {
    out: String,
    next: usize,
}

impl W {
    fn add(&mut self, text: &str) -> usize {
        self.next += 1;
        let _ = writeln!(self.out, "#{}={text};", self.next);
        self.next
    }
}

/// STEP REAL: always has a decimal point.
fn real(v: f64) -> String {
    let s = format!("{v:.6}");
    if s.trim_start_matches('-').trim_matches(|c| c == '0' || c == '.').is_empty() { "0.".into() } else { s }
}

fn triple(v: DVec3) -> String {
    format!("({},{},{})", real(v.x), real(v.y), real(v.z))
}

/// A STEP string body: quotes doubled.
fn q(s: &str) -> String {
    s.replace('\'', "''")
}

fn refs(ids: &[usize]) -> String {
    ids.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(",")
}

/// Writes one FACETED_BREP (one planar FACE_SURFACE per triangle, as before) and returns its id.
fn faceted_brep(w: &mut W, id: u64, body: &Body) -> usize {
    let m = &body.mesh;
    let points: Vec<usize> = m.positions.iter().map(|p| w.add(&format!("CARTESIAN_POINT('',{})", triple(*p)))).collect();
    let mut faces = Vec::new();
    for t in m.indices.as_chunks::<3>().0 {
        let [pa, pb, pc] = t.map(|i| m.positions[i as usize]);
        let cross = (pb - pa).cross(pc - pa);
        if cross.length() < 1e-12 {
            continue;
        }
        let (n, r) = (cross.normalize(), (pb - pa).normalize());
        let dn = w.add(&format!("DIRECTION('',{})", triple(n)));
        let dr = w.add(&format!("DIRECTION('',{})", triple(r)));
        let place = w.add(&format!("AXIS2_PLACEMENT_3D('',#{},#{dn},#{dr})", points[t[0] as usize]));
        let plane = w.add(&format!("PLANE('',#{place})"));
        let corners = refs(&t.map(|i| points[i as usize]));
        let lp = w.add(&format!("POLY_LOOP('',({corners}))"));
        let bound = w.add(&format!("FACE_OUTER_BOUND('',#{lp},.T.)"));
        faces.push(w.add(&format!("FACE_SURFACE('',(#{bound}),#{plane},.T.)")));
    }
    let shell = w.add(&format!("CLOSED_SHELL('',({}))", refs(&faces)));
    w.add(&format!("FACETED_BREP('Body_{id}',#{shell})"))
}

/// `.T.` or `.F.`
fn flag(b: bool) -> &'static str {
    if b { ".T." } else { ".F." }
}

fn direction(w: &mut W, d: DVec3) -> usize {
    w.add(&format!("DIRECTION('',{})", triple(d.normalize())))
}

/// AXIS2_PLACEMENT_3D with its point and two directions; returns its id.
fn place(w: &mut W, origin: DVec3, z: DVec3, x: DVec3) -> usize {
    let o = w.add(&format!("CARTESIAN_POINT('',{})", triple(origin)));
    let (dz, dx) = (direction(w, z), direction(w, x));
    w.add(&format!("AXIS2_PLACEMENT_3D('',#{o},#{dz},#{dx})"))
}

/// Writes one MANIFOLD_SOLID_BREP of ADVANCED_FACEs and returns its id.
fn advanced_brep(w: &mut W, id: u64, t: &TrimmedBody) -> usize {
    let vertex_points: Vec<usize> = t.vertices.iter().map(|p| w.add(&format!("CARTESIAN_POINT('',{})", triple(*p)))).collect();
    let vertices: Vec<usize> = vertex_points.iter().map(|p| w.add(&format!("VERTEX_POINT('',#{p})"))).collect();
    let mut curves = Vec::new();
    for e in &t.edges {
        let (p0, p1) = (t.vertices[e.vertices[0] as usize], t.vertices[e.vertices[1] as usize]);
        let curve = match &e.geom {
            EdgeGeom::Line { .. } => {
                let dir = direction(w, p1 - p0);
                let vec = w.add(&format!("VECTOR('',#{dir},{})", real(p0.distance(p1))));
                w.add(&format!("LINE('',#{},#{vec})", vertex_points[e.vertices[0] as usize]))
            }
            EdgeGeom::Circle { center, axis, radius, .. } => {
                let x = (p0 - *center).reject_from(*axis).normalize();
                let pl = place(w, *center, *axis, x);
                w.add(&format!("CIRCLE('',#{pl},{})", real(*radius)))
            }
            EdgeGeom::Ellipse { center, axis, major_dir, major, minor } => {
                let pl = place(w, *center, *axis, *major_dir);
                w.add(&format!("ELLIPSE('',#{pl},{},{})", real(*major), real(*minor)))
            }
            EdgeGeom::Other => unreachable!("TrimmedBody has no other edges"),
        };
        curves.push(w.add(&format!("EDGE_CURVE('',#{},#{},#{curve},.T.)", vertices[e.vertices[0] as usize], vertices[e.vertices[1] as usize])));
    }
    let mut faces = Vec::new();
    for f in &t.faces {
        let surface = match &f.surface {
            Surface::Plane { origin, normal } => {
                let pl = place(w, *origin, *normal, normal.any_orthonormal_vector());
                w.add(&format!("PLANE('',#{pl})"))
            }
            Surface::Cylinder { origin, axis, radius } => {
                let pl = place(w, *origin, *axis, axis.any_orthonormal_vector());
                w.add(&format!("CYLINDRICAL_SURFACE('',#{pl},{})", real(*radius)))
            }
            _ => unreachable!("TrimmedBody has only planes and cylinders"),
        };
        let mut bounds = Vec::new();
        for (i, lp) in f.loops.iter().enumerate() {
            let oriented: Vec<usize> = lp.iter().map(|&(e, fwd)| w.add(&format!("ORIENTED_EDGE('',*,*,#{},{})", curves[e as usize], flag(fwd)))).collect();
            let el = w.add(&format!("EDGE_LOOP('',({}))", refs(&oriented)));
            let kind = if f.outer == Some(i) { "FACE_OUTER_BOUND" } else { "FACE_BOUND" };
            bounds.push(w.add(&format!("{kind}('',#{el},.T.)")));
        }
        faces.push(w.add(&format!("ADVANCED_FACE('',({}),#{surface},{})", refs(&bounds), flag(f.outward))));
    }
    let shell = w.add(&format!("CLOSED_SHELL('',({}))", refs(&faces)));
    w.add(&format!("MANIFOLD_SOLID_BREP('Body_{id}',#{shell})"))
}

/// One solid per body; `name` is the product name. A body of planes and cylinders becomes a MANIFOLD_SOLID_BREP
/// of ADVANCED_FACEs; any other body a FACETED_BREP whose triangles are planar FACE_SURFACEs bounded by a POLY_LOOP.
// shortcut: bodies with cones, spheres, tori, free-form faces or other curves are written faceted; add those surfaces when users need them
pub fn to_step(bodies: &[(u64, &Body)], name: &str) -> String {
    let name = q(name);
    let mut w = W { out: String::new(), next: 0 };
    let _ = write!(
        w.out,
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ProjectCAD solid, units mm'),'2;1');\nFILE_NAME('{name}.step','',(''),(''),'ProjectCAD','ProjectCAD','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN {{ 1 0 10303 214 1 1 1 1 }}'));\nENDSEC;\nDATA;\n"
    );
    let a = w.add("APPLICATION_CONTEXT('automotive design')");
    w.add(&format!("APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{a})"));
    let pc = w.add(&format!("PRODUCT_CONTEXT('',#{a},'mechanical')"));
    let p = w.add(&format!("PRODUCT('{name}','{name}','',(#{pc}))"));
    let pf = w.add(&format!("PRODUCT_DEFINITION_FORMATION('','',#{p})"));
    let dc = w.add(&format!("PRODUCT_DEFINITION_CONTEXT('part definition',#{a},'design')"));
    let pd = w.add(&format!("PRODUCT_DEFINITION('design','',#{pf},#{dc})"));
    let ps = w.add(&format!("PRODUCT_DEFINITION_SHAPE('','',#{pd})"));
    let lu = w.add("(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))");
    let au = w.add("(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))");
    let su = w.add("(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())");
    let un = w.add(&format!("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-06),#{lu},'distance_accuracy_value','')"));
    let ctx = w.add(&format!(
        "(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{un}))GLOBAL_UNIT_ASSIGNED_CONTEXT((#{lu},#{au},#{su}))REPRESENTATION_CONTEXT('',''))"
    ));
    let o = w.add("CARTESIAN_POINT('',(0.,0.,0.))");
    let z = w.add("DIRECTION('',(0.,0.,1.))");
    let x = w.add("DIRECTION('',(1.,0.,0.))");
    let ax = w.add(&format!("AXIS2_PLACEMENT_3D('',#{o},#{z},#{x})"));
    let (mut advanced, mut faceted) = (Vec::new(), Vec::new());
    for (id, body) in bodies {
        match TrimmedBody::from_body(body) {
            Ok(t) => advanced.push(advanced_brep(&mut w, *id, &t)),
            Err(_) => faceted.push(faceted_brep(&mut w, *id, body)),
        }
    }
    let shape_rep = |w: &mut W, kind: &str, items: &[usize]| {
        let all: Vec<usize> = std::iter::once(ax).chain(items.iter().copied()).collect();
        w.add(&format!("{kind}('',({}),#{ctx})", refs(&all)))
    };
    let rep = if faceted.is_empty() {
        shape_rep(&mut w, "ADVANCED_BREP_SHAPE_REPRESENTATION", &advanced)
    } else if advanced.is_empty() {
        shape_rep(&mut w, "FACETED_BREP_SHAPE_REPRESENTATION", &faceted)
    } else {
        let main = w.add(&format!("SHAPE_REPRESENTATION('',(#{ax}),#{ctx})"));
        for (kind, items) in [("ADVANCED_BREP_SHAPE_REPRESENTATION", &advanced), ("FACETED_BREP_SHAPE_REPRESENTATION", &faceted)] {
            let r = shape_rep(&mut w, kind, items);
            w.add(&format!("SHAPE_REPRESENTATION_RELATIONSHIP('','',#{main},#{r})"));
        }
        main
    };
    w.add(&format!("SHAPE_DEFINITION_REPRESENTATION(#{ps},#{rep})"));
    w.out.push_str("ENDSEC;
END-ISO-10303-21;
");
    w.out
}

#[cfg(test)]
#[path = "step_out_tests.rs"]
mod tests;
