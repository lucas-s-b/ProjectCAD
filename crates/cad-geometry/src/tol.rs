#[derive(Debug, Clone, Copy)]
pub struct Tolerance {
    pub linear: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self { linear: 1e-9 }
    }
}
