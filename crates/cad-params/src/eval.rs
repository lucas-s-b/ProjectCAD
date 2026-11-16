use crate::parser::{BinOp, Expr, Func};
use crate::value::{Dim, ParamError, Value};

type Res = Result<Value, ParamError>;

impl Expr {
    pub fn eval(&self, lookup: &dyn Fn(&str) -> Option<Value>) -> Result<Value, ParamError> {
        match self {
            Expr::Num(v) => Ok(*v),
            Expr::Var(n) => lookup(n).ok_or_else(|| ParamError::UnknownName(n.clone())),
            Expr::Neg(a) => {
                let a = a.eval(lookup)?;
                finite(-a.v, a.dim)
            }
            Expr::Binary(op, a, b) => binary(*op, a.eval(lookup)?, b.eval(lookup)?),
            Expr::Call(Func::If, args) => {
                let c = args[0].eval(lookup)?;
                if c.dim != Dim::SCALAR {
                    return Err(ParamError::UnitMismatch { op: "if" });
                }
                args[if c.v != 0.0 { 1 } else { 2 }].eval(lookup)
            }
            Expr::Call(f, args) => {
                let vals = args.iter().map(|a| a.eval(lookup)).collect::<Result<Vec<_>, _>>()?;
                call(*f, &vals)
            }
        }
    }
}

fn finite(v: f64, dim: Dim) -> Res {
    if v.is_finite() { Ok(Value { v, dim }) } else { Err(ParamError::Domain("result is not finite")) }
}

fn flag(b: bool) -> Res {
    Ok(Value { v: if b { 1.0 } else { 0.0 }, dim: Dim::SCALAR })
}

/// Makes two operands share a dimension; a lone scalar adopts the other side's dimension.
fn compat(a: Value, b: Value, op: &'static str) -> Result<(Value, Value), ParamError> {
    if a.dim == b.dim {
        Ok((a, b))
    } else if a.dim == Dim::SCALAR {
        Ok((a.coerce(b.dim).expect("scalar coerces"), b))
    } else if b.dim == Dim::SCALAR {
        Ok((a, b.coerce(a.dim).expect("scalar coerces")))
    } else {
        Err(ParamError::UnitMismatch { op })
    }
}

fn binary(op: BinOp, a: Value, b: Value) -> Res {
    let name = match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Pow => "^",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
    };
    match op {
        BinOp::Add | BinOp::Sub => {
            let (a, b) = compat(a, b, name)?;
            finite(if op == BinOp::Add { a.v + b.v } else { a.v - b.v }, a.dim)
        }
        BinOp::Mul => finite(a.v * b.v, add_dims(a.dim, b.dim, 1)?),
        BinOp::Div => {
            if b.v == 0.0 {
                return Err(ParamError::Domain("division by zero"));
            }
            finite(a.v / b.v, add_dims(a.dim, b.dim, -1)?)
        }
        BinOp::Pow => power(a, b),
        _ => {
            let (a, b) = compat(a, b, name)?;
            let eq = a.v == b.v || (a.v - b.v).abs() <= 1e-12 * a.v.abs().max(b.v.abs());
            flag(match op {
                BinOp::Lt => a.v < b.v,
                BinOp::Le => a.v <= b.v,
                BinOp::Gt => a.v > b.v,
                BinOp::Ge => a.v >= b.v,
                BinOp::Eq => eq,
                _ => !eq,
            })
        }
    }
}

/// `a + sign * b` per exponent.
fn add_dims(a: Dim, b: Dim, sign: i8) -> Result<Dim, ParamError> {
    let too_large = || ParamError::Domain("exponent too large");
    Ok(Dim {
        length: a.length.checked_add(sign * b.length).ok_or_else(too_large)?,
        angle: a.angle.checked_add(sign * b.angle).ok_or_else(too_large)?,
    })
}

fn power(base: Value, exp: Value) -> Res {
    if exp.dim != Dim::SCALAR {
        return Err(ParamError::UnitMismatch { op: "^" });
    }
    if base.dim == Dim::SCALAR {
        return finite(base.v.powf(exp.v), Dim::SCALAR);
    }
    let n = exp.v.round();
    if (exp.v - n).abs() > 1e-9 {
        return Err(ParamError::UnitMismatch { op: "^" });
    }
    let scale = |e: i8| -> Result<i8, ParamError> {
        let p = f64::from(e) * n;
        if (-128.0..=127.0).contains(&p) { Ok(p as i8) } else { Err(ParamError::Domain("exponent too large")) }
    };
    let dim = Dim { length: scale(base.dim.length)?, angle: scale(base.dim.angle)? };
    finite(base.v.powf(n), dim)
}

fn call(f: Func, a: &[Value]) -> Res {
    let name = f.name();
    let angle = |v: Value| v.as_angle_rad().ok_or(ParamError::UnitMismatch { op: name });
    let scalar = |v: Value| if v.dim == Dim::SCALAR { Ok(v.v) } else { Err(ParamError::UnitMismatch { op: name }) };
    let sc = |v: f64| finite(v, Dim::SCALAR);
    match f {
        Func::Sin => sc(angle(a[0])?.sin()),
        Func::Cos => sc(angle(a[0])?.cos()),
        Func::Tan => sc(angle(a[0])?.tan()),
        Func::Asin | Func::Acos => {
            let x = scalar(a[0])?;
            if !(-1.0..=1.0).contains(&x) {
                return Err(ParamError::Domain("argument outside [-1, 1]"));
            }
            finite(if f == Func::Asin { x.asin() } else { x.acos() }, Dim::ANGLE)
        }
        Func::Atan => finite(scalar(a[0])?.atan(), Dim::ANGLE),
        Func::Atan2 => {
            let (y, x) = compat(a[0], a[1], name)?;
            finite(y.v.atan2(x.v), Dim::ANGLE)
        }
        Func::Sqrt => {
            let x = a[0];
            if x.v < 0.0 {
                return Err(ParamError::Domain("square root of a negative number"));
            }
            if x.dim.length % 2 != 0 || x.dim.angle % 2 != 0 {
                return Err(ParamError::UnitMismatch { op: name });
            }
            finite(x.v.sqrt(), Dim { length: x.dim.length / 2, angle: x.dim.angle / 2 })
        }
        Func::Abs => finite(a[0].v.abs(), a[0].dim),
        Func::Min | Func::Max => {
            let (x, y) = compat(a[0], a[1], name)?;
            finite(if f == Func::Min { x.v.min(y.v) } else { x.v.max(y.v) }, x.dim)
        }
        Func::Floor | Func::Ceil | Func::Round => {
            let x = a[0];
            if x.dim != Dim::SCALAR && x.dim != Dim::LENGTH {
                return Err(ParamError::UnitMismatch { op: name });
            }
            finite(
                match f {
                    Func::Floor => x.v.floor(),
                    Func::Ceil => x.v.ceil(),
                    _ => x.v.round(),
                },
                x.dim,
            )
        }
        Func::If => unreachable!("if is evaluated lazily"),
    }
}

#[cfg(test)]
#[path = "eval_tests.rs"]
mod tests;
