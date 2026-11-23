mod jacobian;
mod residual;
pub mod solve;
pub mod types;

#[cfg(test)]
mod pinned_tests;
#[cfg(test)]
mod residual_tests;
#[cfg(test)]
mod jacobian_tests;
#[cfg(test)]
mod solve_tests;

pub use faer::sparse::Triplet;
pub use solve::{Fit, Solution, Status, least_squares, solve, solve_pinned};
pub use types::{Constraint, Id, PointKind, PointRef, SolveError};
