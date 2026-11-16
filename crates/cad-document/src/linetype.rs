use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LineType {
    #[default]
    Continuous,
    Dashed,
    Hidden,
    Center,
}

impl LineType {
    pub const ALL: [LineType; 4] = [LineType::Continuous, LineType::Dashed, LineType::Hidden, LineType::Center];

    pub fn name(self) -> &'static str {
        match self {
            LineType::Continuous => "Continuous",
            LineType::Dashed => "Dashed",
            LineType::Hidden => "Hidden",
            LineType::Center => "Center",
        }
    }

    /// Dash, gap, dash, gap... in millimetres on paper; empty for a continuous line.
    pub fn pattern_mm(self) -> &'static [f64] {
        match self {
            LineType::Continuous => &[],
            LineType::Dashed => &[3.0, 1.5],
            LineType::Hidden => &[1.5, 1.0],
            LineType::Center => &[8.0, 1.5, 1.5, 1.5],
        }
    }
}
