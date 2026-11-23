//! The model of the example plugin `examples/bolt_circle.rs`: a round flange with a circle of bolt holes, as document
//! commands. In the library so its tests run (tests of example targets do not run by default).
use cad_document::{BodyOp, Command, Entity, Feature, FeatureKind, NextIds, Target};
use cad_geometry::{DVec2, Shape};
use std::path::PathBuf;

/// A flange on the XY plane (sketch 0), centred on the origin, `thickness` up from z = 0, with `count` holes on a
/// circle; millimetres.
#[derive(Clone, Debug, PartialEq)]
pub struct BoltCircle {
    pub count: u32,
    pub bolt_radius: f64,
    pub hole_diameter: f64,
    pub outer_radius: f64,
    pub thickness: f64,
}

impl Default for BoltCircle {
    fn default() -> Self {
        BoltCircle { count: 6, bolt_radius: 40.0, hole_diameter: 8.0, outer_radius: 60.0, thickness: 10.0 }
    }
}

/// The example's command line.
#[derive(Clone, Debug, PartialEq)]
pub struct Args {
    pub flange: BoltCircle,
    /// `--app`: the app to start when not launched from the Plugins menu.
    pub app: Option<PathBuf>,
    /// `--save`: where a standalone run saves the project.
    pub save: Option<PathBuf>,
}

impl BoltCircle {
    /// Err unless every size is finite and positive, 1 <= count <= 360, the holes stay inside the rim
    /// (bolt_radius + hole_diameter / 2 < outer_radius) and off the centre (bolt_radius > hole_diameter / 2), and,
    /// for two or more holes, neighbours do not touch (2 * bolt_radius * sin(pi / count) > hole_diameter).
    pub fn check(&self) -> Result<(), String> {
        for (name, v) in [("bolt radius", self.bolt_radius), ("hole diameter", self.hole_diameter), ("outer radius", self.outer_radius), ("thickness", self.thickness)] {
            if !v.is_finite() || v <= 0.0 {
                return Err(format!("{name} must be a positive number"));
            }
        }
        if !(1..=360).contains(&self.count) {
            return Err("count must be 1 to 360".into());
        }
        let r = self.hole_diameter / 2.0;
        if self.bolt_radius + r >= self.outer_radius {
            return Err("the holes must stay inside the rim (bolt radius + hole radius < outer radius)".into());
        }
        if self.bolt_radius <= r {
            return Err("the holes must stay off the centre (bolt radius > hole radius)".into());
        }
        if self.count >= 2 && 2.0 * self.bolt_radius * (std::f64::consts::PI / f64::from(self.count)).sin() <= self.hole_diameter {
            return Err("neighbouring holes touch".into());
        }
        Ok(())
    }

    /// The commands that add the flange, numbered from `next`, its features appended after `features` existing ones:
    /// the outline circle (entity `next.entity`, "Flange outline"), the holes (the following entity ids, "Bolt hole 1"…,
    /// the first on +X, counter-clockwise), the disc (feature `next.feature` at index `features`, "Flange", extrude
    /// `BodyOp::New`) and one cutting extrude per hole (features `next.feature + 1` and up, "Bolt hole 1"…, `BodyOp::Cut`;
    /// the rebuild takes one region per extrude, so one cut over several disjoint circles is refused).
    pub fn commands(&self, next: &NextIds, features: usize) -> Vec<Command> {
        let circle = |id, name: String, center, radius| {
            Command::Add(Entity { id, name, shape: Shape::Circle { center, radius }, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })
        };
        let extrude = |id, name: &str, profile: Vec<u64>, op| Feature {
            id,
            name: name.into(),
            suppressed: false,
            kind: FeatureKind::Extrude {
                profile,
                distance: format!("{} mm", self.thickness),
                offset: "0".into(),
                taper: "0 deg".into(),
                op,
                flip: false,
                target: Target::Auto,
            },
        };
        let mut cmds = vec![circle(next.entity, "Flange outline".into(), DVec2::ZERO, self.outer_radius)];
        for i in 0..self.count {
            let id = next.entity + 1 + u64::from(i);
            let a = std::f64::consts::TAU * f64::from(i) / f64::from(self.count);
            cmds.push(circle(id, format!("Bolt hole {}", i + 1), DVec2::new(self.bolt_radius * a.cos(), self.bolt_radius * a.sin()), self.hole_diameter / 2.0));
        }
        cmds.push(Command::AddFeature { index: features, feature: extrude(next.feature, "Flange", vec![next.entity], BodyOp::New) });
        for i in 0..u64::from(self.count) {
            let name = format!("Bolt hole {}", i + 1);
            let feature = extrude(next.feature + 1 + i, &name, vec![next.entity + 1 + i], BodyOp::Cut);
            cmds.push(Command::AddFeature { index: features + 1 + i as usize, feature });
        }
        cmds
    }
}

/// `--count N --bolt-radius MM --hole-diameter MM --outer-radius MM --thickness MM --app PATH --save PATH`, each
/// optional; Err("unknown argument <a>"), Err("missing value for <flag>") or Err("<flag>: <parse error>").
pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args { flange: BoltCircle::default(), app: None, save: None };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("missing value for {flag}"));
        let num = |text: &String| text.parse::<f64>().map_err(|err| format!("{flag}: {err}"));
        match flag.as_str() {
            "--count" => out.flange.count = value()?.parse().map_err(|err| format!("{flag}: {err}"))?,
            "--bolt-radius" => out.flange.bolt_radius = num(value()?)?,
            "--hole-diameter" => out.flange.hole_diameter = num(value()?)?,
            "--outer-radius" => out.flange.outer_radius = num(value()?)?,
            "--thickness" => out.flange.thickness = num(value()?)?,
            "--app" => out.app = Some(value()?.into()),
            "--save" => out.save = Some(value()?.into()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "bolt_circle_tests.rs"]
mod tests;
