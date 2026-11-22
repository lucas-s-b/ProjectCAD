//! Deep groove ball bearings, ISO 15 dimension series 10 (60xx) and 02 (62xx): an inner and an outer ring with a ball
//! groove each and a full set of balls; no cage, seals or ring chamfers. The internal geometry is derived from the
//! boundary dimensions (ISO 15 fixes only bore, outside diameter and width).
use crate::num;
use cad_geometry::profile::{Profile, Seg};
use cad_geometry::{DAffine3, DVec2, DVec3};
use cad_modeler::{Body, BodyFace, ModelError, Plane3, Surface, lathe, revolve, union_all};
use std::f64::consts::{PI, TAU};

/// One size: designation, bore d, outside diameter D and width B (ISO 15). Millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BearingSize {
    pub name: &'static str,
    pub bore: f64,
    pub outer: f64,
    pub width: f64,
}

const fn row(name: &'static str, bore: f64, outer: f64, width: f64) -> BearingSize {
    BearingSize { name, bore, outer, width }
}

/// The supported sizes: 6000 to 6010, then 6200 to 6210.
pub const BEARINGS: [BearingSize; 22] = [
    row("6000", 10.0, 26.0, 8.0),
    row("6001", 12.0, 28.0, 8.0),
    row("6002", 15.0, 32.0, 9.0),
    row("6003", 17.0, 35.0, 10.0),
    row("6004", 20.0, 42.0, 12.0),
    row("6005", 25.0, 47.0, 12.0),
    row("6006", 30.0, 55.0, 13.0),
    row("6007", 35.0, 62.0, 14.0),
    row("6008", 40.0, 68.0, 15.0),
    row("6009", 45.0, 75.0, 16.0),
    row("6010", 50.0, 80.0, 16.0),
    row("6200", 10.0, 30.0, 9.0),
    row("6201", 12.0, 32.0, 10.0),
    row("6202", 15.0, 35.0, 11.0),
    row("6203", 17.0, 40.0, 12.0),
    row("6204", 20.0, 47.0, 14.0),
    row("6205", 25.0, 52.0, 15.0),
    row("6206", 30.0, 62.0, 16.0),
    row("6207", 35.0, 72.0, 17.0),
    row("6208", 40.0, 80.0, 18.0),
    row("6209", 45.0, 85.0, 19.0),
    row("6210", 50.0, 90.0, 20.0),
];

/// The size called `name` ("6204"; surrounding spaces are ignored).
pub fn bearing(name: &str) -> Option<BearingSize> {
    let name = name.trim();
    BEARINGS.iter().find(|b| b.name == name).copied()
}

/// A deep groove ball bearing lying on the plane from height 0 to its width, its axis the plane normal through the
/// plane origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bearing {
    pub size: BearingSize,
}

impl Bearing {
    /// Ball diameter Dw = 0.3 (D - d).
    pub fn ball(&self) -> f64 {
        0.3 * (self.size.outer - self.size.bore)
    }

    /// Diameter of the circle through the ball centres, (D + d) / 2.
    pub fn pitch_diameter(&self) -> f64 {
        (self.size.outer + self.size.bore) / 2.0
    }

    /// As many balls as fit with centres 1.6 Dw apart along the pitch circle, at least 3.
    pub fn balls(&self) -> usize {
        ((PI * self.pitch_diameter() / (1.6 * self.ball())).floor() as usize).max(3)
    }

    /// "Ball bearing 6204 20x47x14": designation, bore, outside diameter, width.
    pub fn designation(&self) -> String {
        let s = self.size;
        format!("Ball bearing {} {}x{}x{}", s.name, num(s.bore), num(s.outer), num(s.width))
    }

    /// Both rings (revolved sections with a groove of radius 0.52 Dw about the ball centre circle, shoulders at
    /// pitch radius -+ 0.3 Dw) and the balls (turned within `tol`, one sphere face each), united into one body. The
    /// grooves are 4 % wider than the balls, so nothing touches.
    pub fn body(&self, plane: &Plane3, tol: f64) -> Result<Body, ModelError> {
        let BearingSize { bore, outer, width, .. } = self.size;
        let (dw, rp) = (self.ball(), self.pitch_diameter() / 2.0);
        let (rg, h) = (0.52 * dw, 0.3 * dw);
        // Half the angle the groove arc spans about its centre: it meets a shoulder h from the centre.
        let alpha = (h / rg).acos();
        let c = DVec2::new(rp, width / 2.0);
        let p = DVec2::new;
        let line = |a: DVec2, b: DVec2| Seg::Line { a, b };
        // Sections in (radius, height), counter-clockwise. The inner groove bulges towards the axis, the outer away.
        let (ri, ro, r0, r1) = (rp - h, rp + h, bore / 2.0, outer / 2.0);
        let g = Seg::Arc { center: c, radius: rg, start: PI + alpha, sweep: -2.0 * alpha };
        let (gs, ge) = (g.start(), g.end());
        let inner = vec![line(p(r0, 0.0), p(ri, 0.0)), line(p(ri, 0.0), gs), g, line(ge, p(ri, width)), line(p(ri, width), p(r0, width)), line(p(r0, width), p(r0, 0.0))];
        let g = Seg::Arc { center: c, radius: rg, start: alpha, sweep: -2.0 * alpha };
        let (gs, ge) = (g.start(), g.end());
        let outer_ring = vec![line(p(ro, 0.0), p(r1, 0.0)), line(p(r1, 0.0), p(r1, width)), line(p(r1, width), p(ro, width)), line(p(ro, width), gs), g, line(ge, p(ro, 0.0))];
        // The section plane holds the axis: x radial along plane.x, y along the plane normal.
        let section = Plane3 { origin: plane.origin, x: plane.x, y: plane.normal() };
        let ring = |l: Vec<Seg>| -> Result<Body, ModelError> {
            Body::from_brep(&revolve(&Profile { outer: l, holes: Vec::new() }, &section, DVec2::ZERO, DVec2::Y, TAU)?, tol)
        };
        let mut parts = vec![ring(inner)?, ring(outer_ring)?];
        // One ball at the origin: a half circle turned with n facets per turn (within tol, at least 16, even).
        let r = dw / 2.0;
        let n = ((PI / (1.0 - tol / r).clamp(-1.0, 1.0).acos()).ceil() as usize).max(16).next_multiple_of(2);
        let half = n / 2;
        let mut outline = vec![p(0.0, -r)];
        outline.extend((1..half).map(|i| {
            let a = PI * i as f64 / half as f64;
            p(r * a.sin(), -r * a.cos())
        }));
        outline.push(p(0.0, r));
        let mut ball = lathe(&outline, &Plane3 { origin: DVec3::ZERO, ..*plane }, n)?;
        // One sphere face instead of a cone per outline edge (no latitude lines); `transformed` recomputes the normals.
        ball.faces = vec![BodyFace { surface: Surface::Sphere { center: DVec3::ZERO, radius: r }, outward: true }];
        ball.tri_face.fill(0);
        let z = self.balls();
        for i in 0..z {
            let a = TAU * i as f64 / z as f64;
            let center = plane.origin + (plane.x * a.cos() + plane.y * a.sin()) * rp + plane.normal() * (width / 2.0);
            parts.push(ball.transformed(&DAffine3::from_translation(center))?);
        }
        union_all(&parts)?.ok_or(ModelError::Invalid("bearing has no parts"))
    }
}

#[cfg(test)]
#[path = "bearing_tests.rs"]
mod tests;
