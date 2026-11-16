mod residual;
pub mod solve;
pub mod types;

#[cfg(test)]
mod residual_tests;
#[cfg(test)]
mod solve_tests;

pub use solve::{Solution, Status, solve};
pub use types::{Constraint, Id, PointKind, PointRef, SolveError};
