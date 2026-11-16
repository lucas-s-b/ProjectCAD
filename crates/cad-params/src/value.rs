use std::f64::consts::PI;

/// Dimension exponents of a quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dim {
    pub length: i8,
    pub angle: i8,
}

impl Dim {
    pub const SCALAR: Dim = Dim { length: 0, angle: 0 };
    pub const LENGTH: Dim = Dim { length: 1, angle: 0 };
    pub const ANGLE: Dim = Dim { length: 0, angle: 1 };
}

/// A quantity in base units: millimetres and radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Value {
    pub v: f64,
    pub dim: Dim,
}

impl Value {
    /// A bare number means millimetres where a length is expected and degrees where an angle is expected.
    pub fn coerce(self, to: Dim) -> Option<Value> {
        if self.dim == to {
            Some(self)
        } else if self.dim == Dim::SCALAR {
            let v = if to == Dim::ANGLE { self.v * PI / 180.0 } else { self.v };
            Some(Value { v, dim: to })
        } else {
            None
        }
    }

    pub fn as_length_mm(self) -> Option<f64> {
        self.coerce(Dim::LENGTH).map(|x| x.v)
    }

    pub fn as_angle_rad(self) -> Option<f64> {
        self.coerce(Dim::ANGLE).map(|x| x.v)
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParamError {
    #[error("syntax error at {pos}: {msg}")]
    Syntax { pos: usize, msg: String },
    #[error("unknown name '{0}'")]
    UnknownName(String),
    #[error("unit mismatch in {op}")]
    UnitMismatch { op: &'static str },
    #[error("{func} takes {expected} argument(s), got {got}")]
    BadArgs { func: &'static str, expected: usize, got: usize },
    #[error("math error: {0}")]
    Domain(&'static str),
    #[error("circular reference: {}", .0.join(" -> "))]
    Cycle(Vec<String>),
    #[error("'{0}' is not a valid parameter name")]
    InvalidName(String),
    #[error("depends on '{0}', which has an error")]
    Depends(String),
}

/// Length unit suffix -> millimetres, angle unit suffix -> radians.
pub fn unit(name: &str) -> Option<Value> {
    let (v, dim) = match name {
        "mm" => (1.0, Dim::LENGTH),
        "cm" => (10.0, Dim::LENGTH),
        "m" => (1000.0, Dim::LENGTH),
        "in" => (25.4, Dim::LENGTH),
        "ft" => (304.8, Dim::LENGTH),
        "deg" => (PI / 180.0, Dim::ANGLE),
        "rad" => (1.0, Dim::ANGLE),
        _ => return None,
    };
    Some(Value { v, dim })
}
