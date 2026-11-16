use std::f64::consts::PI;

use crate::parse;
use crate::value::{Dim, ParamError, Value};

fn lookup(n: &str) -> Option<Value> {
    match n {
        "width" => Some(Value { v: 100.0, dim: Dim::LENGTH }),
        "angle" => Some(Value { v: PI / 2.0, dim: Dim::ANGLE }),
        _ => None,
    }
}

fn ev(src: &str) -> Result<Value, ParamError> {
    parse(src).unwrap().eval(&lookup)
}

fn ok(src: &str) -> Value {
    ev(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

fn assert_val(src: &str, v: f64, dim: Dim) {
    let r = ok(src);
    assert!((r.v - v).abs() < 1e-9, "{src}: {} != {v}", r.v);
    assert_eq!(r.dim, dim, "{src}");
}

const S: Dim = Dim::SCALAR;
const L: Dim = Dim::LENGTH;
const A: Dim = Dim::ANGLE;

#[test]
fn arithmetic_with_units() {
    assert_val("100 mm + 2 in", 150.8, L);
    assert_val("1 m / 4", 250.0, L);
    assert_val("10 mm * 20 mm", 200.0, Dim { length: 2, angle: 0 });
    assert_val("sqrt(100 mm * 4 mm)", 20.0, L);
    assert_val("1 + 2 * 3", 7.0, S);
    assert_val("-2^2", -4.0, S);
    assert_val("2^3^2", 512.0, S);
    assert_val("10 mm / 2 mm", 5.0, S);
}

#[test]
fn scalars_adopt_other_dim() {
    assert_val("width + 5", 105.0, L);
    assert_val("5 + width", 105.0, L);
    assert_val("angle + 5", PI / 2.0 + 5.0 * PI / 180.0, A);
    assert_val("width - 5", 95.0, L);
    assert_eq!(ev("45 deg + 10 mm"), Err(ParamError::UnitMismatch { op: "+" }));
    assert_eq!(ev("45 deg - 10 mm"), Err(ParamError::UnitMismatch { op: "-" }));
}

#[test]
fn trig() {
    assert_val("sin(30)", 0.5, S);
    assert_val("sin(30 deg)", 0.5, S);
    assert_val("sin(0.5235987755982988 rad)", 0.5, S);
    assert_val("cos(60)", 0.5, S);
    assert_val("tan(45)", 1.0, S);
    assert_val("asin(1)", PI / 2.0, A);
    assert_val("acos(1)", 0.0, A);
    assert_val("atan(1)", PI / 4.0, A);
    assert_val("atan2(1, 1)", PI / 4.0, A);
    assert_val("atan2(1 mm, 1)", PI / 4.0, A);
    assert_eq!(ev("sin(1 mm)"), Err(ParamError::UnitMismatch { op: "sin" }));
    assert_eq!(ev("atan2(1 mm, 1 deg)"), Err(ParamError::UnitMismatch { op: "atan2" }));
    assert_eq!(ev("asin(2)"), Err(ParamError::Domain("argument outside [-1, 1]")));
    assert_eq!(ev("acos(1 mm)"), Err(ParamError::UnitMismatch { op: "acos" }));
}

#[test]
fn powers() {
    assert_val("2^10", 1024.0, S);
    assert_val("2^0.5", 2f64.sqrt(), S);
    assert_val("(2 mm)^2", 4.0, Dim { length: 2, angle: 0 });
    assert_val("(2 mm)^-1", 0.5, Dim { length: -1, angle: 0 });
    assert!(ev("(2 mm)^0.5").is_err());
    assert_eq!(ev("2^(1 mm)"), Err(ParamError::UnitMismatch { op: "^" }));
    assert_eq!(ev("(2 mm)^200"), Err(ParamError::Domain("exponent too large")));
    assert_eq!(ev("10^1000"), Err(ParamError::Domain("result is not finite")));
}

#[test]
fn domain_errors() {
    assert_eq!(ev("sqrt(-1)"), Err(ParamError::Domain("square root of a negative number")));
    assert_eq!(ev("1/0"), Err(ParamError::Domain("division by zero")));
    assert_eq!(ev("1 mm / 0 mm"), Err(ParamError::Domain("division by zero")));
    assert_eq!(ev("sqrt(4 mm)"), Err(ParamError::UnitMismatch { op: "sqrt" }));
    assert_eq!(ev("tan(90)").map(|v| v.v.is_finite()), Ok(true));
}

#[test]
fn if_is_lazy() {
    assert_val("if(1 < 2, 10 mm, 1/0)", 10.0, L);
    assert_val("if(0, 1/0, 3)", 3.0, S);
    assert_val("if(2, 1 mm, 5)", 1.0, L);
    assert_eq!(ev("if(1 mm, 1, 2)"), Err(ParamError::UnitMismatch { op: "if" }));
}

#[test]
fn comparisons() {
    assert_val("1 < 2", 1.0, S);
    assert_val("2 < 1", 0.0, S);
    assert_val("2 <= 2", 1.0, S);
    assert_val("3 > 2", 1.0, S);
    assert_val("2 >= 3", 0.0, S);
    assert_val("1 == 1", 1.0, S);
    assert_val("1 != 1", 0.0, S);
    assert_val("0.1 + 0.2 == 0.3", 1.0, S);
    assert_val("1 m == 1000 mm", 1.0, S);
    assert_val("width > 50", 1.0, S);
    assert_eq!(ev("1 mm < 1 deg"), Err(ParamError::UnitMismatch { op: "<" }));
}

#[test]
fn misc_functions() {
    assert_val("floor(2.7 mm)", 2.0, L);
    assert_val("ceil(2.1)", 3.0, S);
    assert_val("round(2.5 mm)", 3.0, L);
    assert_val("abs(-3 mm)", 3.0, L);
    assert_val("min(1 mm, 2)", 1.0, L);
    assert_val("max(1 mm, 2 cm)", 20.0, L);
    assert_eq!(ev("floor(1 deg)"), Err(ParamError::UnitMismatch { op: "floor" }));
    assert_eq!(ev("min(1 mm, 1 deg)"), Err(ParamError::UnitMismatch { op: "min" }));
}

#[test]
fn variables() {
    assert_val("width * 0.5", 50.0, L);
    assert_eq!(ev("nope + 1"), Err(ParamError::UnknownName("nope".into())));
}
