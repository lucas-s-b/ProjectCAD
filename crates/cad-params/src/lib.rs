//! Parameter expressions: parser, evaluator with units, and dependency-ordered parameter sets.

mod eval;
mod lexer;
mod parser;
mod set;
mod value;

pub use parser::{BinOp, Expr, Func, parse};
pub use set::ParamSet;
pub use value::{Dim, ParamError, Value};
