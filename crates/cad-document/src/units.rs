use cad_geometry::annot::LabelText;
use serde::{Deserialize, Serialize};

/// Display units only; model coordinates are always millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum Units {
    #[default]
    Millimeter,
    Centimeter,
    Meter,
    Inch,
    Foot,
}

impl Units {
    pub const ALL: [Units; 5] = [Units::Millimeter, Units::Centimeter, Units::Meter, Units::Inch, Units::Foot];

    pub fn mm_per_unit(self) -> f64 {
        match self {
            Units::Millimeter => 1.0,
            Units::Centimeter => 10.0,
            Units::Meter => 1000.0,
            Units::Inch => 25.4,
            Units::Foot => 304.8,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Units::Millimeter => "mm",
            Units::Centimeter => "cm",
            Units::Meter => "m",
            Units::Inch => "in",
            Units::Foot => "ft",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub units: Units,
    pub precision: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self { units: Units::Millimeter, precision: 2 }
    }
}

impl Settings {
    /// `mm` converted to the display unit with `precision` decimals, no suffix.
    pub fn format_length(&self, mm: f64) -> String {
        format!("{:.*}", self.precision as usize, mm / self.units.mm_per_unit())
    }

    /// `mm2` (square millimetres) in the display unit squared, with `precision` decimals and the unit suffix.
    pub fn format_area(&self, mm2: f64) -> String {
        let k = self.units.mm_per_unit();
        format!("{:.*} {}\u{b2}", self.precision as usize, mm2 / (k * k), self.units.suffix())
    }

    /// `mm3` (cubic millimetres) in the display unit cubed, with `precision` decimals and the unit suffix.
    pub fn format_volume(&self, mm3: f64) -> String {
        let k = self.units.mm_per_unit();
        format!("{:.*} {}\u{b3}", self.precision as usize, mm3 / (k * k * k), self.units.suffix())
    }

    /// Display text of an annotation label.
    pub fn label_text(&self, t: &LabelText) -> String {
        match t {
            LabelText::Literal(s) => s.clone(),
            LabelText::Length(v) => self.format_length(*v),
            LabelText::Radius(v) => format!("R{}", self.format_length(*v)),
            LabelText::Diameter(v) => format!("\u{d8}{}", self.format_length(*v)),
            LabelText::Angle(a) => format!("{:.*}\u{b0}", self.precision as usize, a.to_degrees()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_volumes() {
        assert_eq!(Settings::default().format_volume(125.0), "125.00 mm\u{b3}");
        assert_eq!(Settings { units: Units::Centimeter, precision: 3 }.format_volume(1500.0), "1.500 cm\u{b3}");
        assert_eq!(Settings { units: Units::Inch, precision: 1 }.format_volume(16387.064), "1.0 in\u{b3}");
    }

    #[test]
    fn formats_in_units() {
        assert_eq!(Settings { units: Units::Inch, precision: 3 }.format_length(25.4), "1.000");
        assert_eq!(Settings { units: Units::Millimeter, precision: 0 }.format_length(25.4), "25");
        assert_eq!(Settings::default().format_length(100.0), "100.00");
        assert_eq!(Settings::default().format_area(2400.0), "2400.00 mm\u{b2}");
        assert_eq!(Settings { units: Units::Meter, precision: 2 }.format_area(12.5e6), "12.50 m\u{b2}");
    }
}

#[cfg(test)]
mod label_tests {
    use super::*;

    #[test]
    fn label_texts() {
        let s = Settings::default();
        assert_eq!(s.label_text(&LabelText::Literal("a".into())), "a");
        assert_eq!(s.label_text(&LabelText::Length(5.0)), "5.00");
        assert_eq!(s.label_text(&LabelText::Radius(20.0)), "R20.00");
        assert_eq!(s.label_text(&LabelText::Diameter(40.0)), "\u{d8}40.00");
        assert_eq!(s.label_text(&LabelText::Angle(std::f64::consts::FRAC_PI_2)), "90.00\u{b0}");
    }
}
