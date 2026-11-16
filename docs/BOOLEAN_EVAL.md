# Boolean engine evaluation (2026-11-15)

Question: can a pure-Rust crate do robust union, difference and intersection on closed triangle meshes?
Spike sources are archived in `docs/boolean-spike/` (`*.txt`, not part of the build). Release build, single runs.
Every result mesh was checked for: closed and manifold after welding at 1e-6, signed volume against an exact or
Monte Carlo reference (tolerance 0.1 %), and degenerate triangles.

## Candidates

| Crate | Outcome |
|---|---|
| manifold-rust 0.19.2 (Apache-2.0, released 2026-11-01) | tested, all cases pass |
| boolmesh 0.2.2 (MPL-2.0, released 2026-10-27) | tested, all cases pass |
| csgrs 0.20.1 (MIT) | tested: volumes right, but the result is not a closed mesh in 9 of 12 cases; does not resolve from crates.io (yanked `core2`) |
| baby_shark 0.3.12 | excluded: C build step through `faer` -> ... -> `psm`; voxel-based anyway |
| cgar 0.2.0 | excluded: mandatory GMP bindings |
| manifold3d, manifold-csg, meshlib bindings | excluded: C++ bindings |
| tri-mesh 0.6.1 | not tested: no boolean operations |

## Cases (manifold-rust and boolmesh gave identical volumes; all closed, no degenerate triangles)

| Case | Result | Relative error | Time ms (manifold-rust / boolmesh) |
|---|---|---|---|
| Union of overlapping boxes | OK | 0 | 0.6 / 0.4 |
| Box minus cylinder | OK | 0 against the faceted reference | 1.3 / 1.7 |
| Cube intersect sphere | OK | -0.34 % (all of it faceting of the sphere) | 4.1 / 5.3 |
| Union of two equal crossing cylinders | OK | +0.002 % | 3.5 / 3.8 |
| Box minus box with coplanar faces | OK | 0 | 0.2 / 0.4 |
| Union of boxes touching on a face | OK | 0 | 0.2 / 0.3 |
| Chained difference | OK | 0 | 4.3 / 5.6 |
| Union of two identical boxes | OK | 0 | 0.2 / 0.2 |
| Box minus box touching along an edge | OK | 0 | 0.2 / 0.2 |
| Union of 30 random boxes, every intermediate checked | OK | 0 | 19 / 26 |
| Overlapping boxes scaled by 1e-3 and by 1e4 | OK | 0 | 0.2 / 0.2 |

## Facts

| | manifold-rust | boolmesh |
|---|---|---|
| Algorithm | Rust port of Manifold 3.5.4 (exact engine) plus a second exact-rational engine; `Auto` picks | own implementation in the style of Manifold |
| Crates added | 12 | 2 (a second glam version) |
| C/C++ in the build | none (`cargo tree -i cc` finds nothing) | none |
| Numeric type | f64 (`MeshGL64`) | f64 |
| Face identity | integer `face_id` per triangle survives the operation (verified) | per-corner float values survive (integer ids need encoding) |

## Decision
`manifold-rust` is the boolean engine. Reasons: Apache-2.0 (no file-level copyleft), a port of a widely used
algorithm with a second engine as fallback, and integer face ids that survive, which the kernel needs to keep CAD
face identity. `boolmesh` is the fallback candidate if `manifold-rust` regresses.

Limit to state everywhere: these booleans work on the tessellation. Curved faces take part as facets at the
tessellation tolerance, and intersection curves come out as polylines. Results keep their source face through
the face id, so the analytic surface of every result triangle is known; exact analytic intersection curves are a
later upgrade.

Both crates are young (first seen in 2026). The spike's cases become regression tests of our boolean layer so a
regression in the dependency is caught.
