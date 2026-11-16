use glam::{DVec2, DVec3};

/// Drawing plane in 3D: `x` and `y` are unit and perpendicular, the normal is `x` cross `y`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane3 {
    pub origin: DVec3,
    pub x: DVec3,
    pub y: DVec3,
}

impl Plane3 {
    pub const XY: Plane3 = Plane3 { origin: DVec3::ZERO, x: DVec3::X, y: DVec3::Y };

    pub fn normal(&self) -> DVec3 {
        self.x.cross(self.y)
    }

    pub fn point(&self, p: DVec2) -> DVec3 {
        self.origin + self.x * p.x + self.y * p.y
    }
}
