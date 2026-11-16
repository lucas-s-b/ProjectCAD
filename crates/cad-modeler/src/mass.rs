use cad_geometry::profile::Profile;

/// Exact volume of an extruded profile, the reference for the mesh volume.
pub fn prism_volume(profile: &Profile, distance: f64) -> f64 {
    profile.area() * distance.abs()
}
