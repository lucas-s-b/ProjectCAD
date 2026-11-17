//! STEP (ISO 10303-21, AP214) export of solid bodies as faceted B-Reps, in millimetres.
use cad_geometry::DVec3;
use cad_modeler::Body;
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

/// One FACETED_BREP per body; `name` is the product name. Each triangle is a FACE_SURFACE on a PLANE bounded by a POLY_LOOP.
// shortcut: one planar face per triangle; merge coplanar triangles per body face when files get large
pub fn to_step(bodies: &[(u64, &Body)], name: &str) -> String {
    let name = q(name);
    let mut w = W { out: String::new(), next: 0 };
    let _ = write!(
        w.out,
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ProjectCAD faceted solid, units mm'),'2;1');\nFILE_NAME('{name}.step','',(''),(''),'ProjectCAD','ProjectCAD','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN {{ 1 0 10303 214 1 1 1 1 }}'));\nENDSEC;\nDATA;\n"
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
    let mut items = vec![ax];
    for (id, body) in bodies {
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
        items.push(w.add(&format!("FACETED_BREP('Body_{id}',#{shell})")));
    }
    let rep = w.add(&format!("FACETED_BREP_SHAPE_REPRESENTATION('',({}),#{ctx})", refs(&items)));
    w.add(&format!("SHAPE_DEFINITION_REPRESENTATION(#{ps},#{rep})"));
    w.out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    w.out
}

#[cfg(test)]
#[path = "step_out_tests.rs"]
mod tests;
