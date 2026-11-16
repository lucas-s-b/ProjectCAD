# Kernel evaluation: truck (2026-11-15)

Spike run by building a throwaway crate against the truck crates and measuring each case against the exact
volume and a closed-mesh check. Sources are archived in `docs/kernel-spike/` (`*.txt`, not part of the build).
Versions: truck-modeling 0.6.0, truck-topology 0.6.0, truck-shapeops 0.4.0, truck-meshalgo 0.4.0, truck-stepio 0.3.0
(newest release in the registry: 2024-09-20). License Apache-2.0. Release build, 12 cores.

## Results

| Case | Result | Relative volume error | Time |
|---|---|---|---|
| Extrude rectangle | OK | 0 | 1 ms |
| Extrude circle, tessellation 0.01 | OK | -7.3e-4 | 8-12 ms |
| Extrude rounded rectangle | OK | -1.6e-4 | 4 ms |
| Extrude rectangle with hole | OK only after inverting the inner wire by hand; wrong orientation is not reported (+18 % volume) | +1.2e-4 | 8 ms |
| Revolve 360 and 90 degrees | Solid comes out inside-out unless the face is inverted first | -1.2e-4 | 2 ms |
| Union of overlapping boxes | OK | 0 | 1 ms |
| Box minus cylinder | OK | +3.1e-4 | 200-700 ms |
| Cube intersect sphere | FAIL (returns None) | - | up to 14 s |
| Union of two equal crossing cylinders | PANIC ("This shell is not oriented and closed") or None | - | 1-2 s |
| Box minus box sharing a coplanar face | FAIL (None) | - | - |
| Union of boxes touching on a face | FAIL (None) | - | - |
| Chained difference | OK | +7.3e-4 | 0.5-1.9 s |
| Fillet, chamfer, shell, offset on solids | No API | - | - |
| STEP export and re-import | OK | - | 1 ms |
| serde of a solid | OK | - | 10 ms |

## Build facts
- `cargo tree -i cc` finds a C build: `lzma-sys` (vendored C) <- `xz2` <- `vtkio` <- `truck-meshalgo`, and
  `truck-shapeops` depends on `truck-meshalgo`. Tessellation and booleans therefore cannot be used without C code.
- 138 crates added. Math types are cgmath f64, not glam.

## Decision
truck is not adopted.
1. It violates the project's absolute rule: no C/C++ in the build.
2. Booleans fail on the cases real parts hit all the time (coplanar faces, touching faces, equal cylinders, sphere).
3. No fillet, chamfer, shell or offset.
4. No release for more than two years.

Consequence: the B-Rep kernel is developed natively (`cad-topology`, `cad-modeler`). The STEP writer of truck is the
one part worth revisiting as a reference when STEP export is built.
