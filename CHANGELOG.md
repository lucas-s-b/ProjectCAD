# Changelog

All notable changes to ProjectCAD. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/).

## [0.2.0] - 2026-11-23

### Added

#### Phase 2: leftovers and constraints
- Construction geometry; split, join, align and distribute; trim, split and intersection snap for ellipses and splines
- Constraints: point on line, point on circle, Symmetric button, constraint glyphs in the 2D view, pinned dragging; constraint values can be parameter expressions
- Groups and object metadata; associative linear dimensions; hatch patterns (lines, cross, solid)
- SVG import and PDF export

#### Phase 3: geometry and parametric modelling
- Sparse constraint solver (a 400-line chain takes 178 ms instead of 3.1 s)
- STEP export: faceted B-Reps, then analytic planes and cylinders with line, circle and ellipse edges
- 3D picking with a BVH, face and edge selection, section planes with caps, and a translate gizmo for Box, Cylinder and Sphere
- Face and edge references that survive rebuilds: sketches on faces follow the face, fillets and chamfers keep their edges
- Tapered extrude and smooth loft through three or more sections
- A rebuild stops when a newer request arrives; a panic in a feature is reported as that feature's error

#### Phase 4: architecture
- Levels, straight walls with automatic joins (L, T, X and hubs), plan outlines and wall solids
- Doors and windows hosted in walls, cut out of the wall solid
- Rooms with areas and perimeters, floors, roofs (flat, shed, gable, hip) and straight stairs
- Multi-floor plan views and "Apply to level"

#### Phase 5: mechanical parts and assemblies
- Part generators in the feature list: involute spur gears, stepped shafts, helical springs, ISO hex bolts and nuts (with optional modelled threads) and deep groove ball bearings
- Components, grounded components, joints (fixed, revolute, slider) with frames taken from faces, and the bill of materials with CSV export
- Exploded views with steps and trail lines
- Interference and clearance checks; dependency graph so a feature recomputes only when what it reads changes; draft of existing planar faces

#### Phase 6: advanced engineering
- Accuracy labels on every analysis result: exact, approximate or validated against a named case
- Mass properties of bodies and assemblies; joint limits; motion studies with a collision check and CSV export
- Closed-loop joints solved by Levenberg-Marquardt; dragging joints in the editor
- Tetrahedral mesher and static linear-elastic FEA with fixed faces, forces, pressures, gravity, a safety factor and a coloured result in 3D
- Validation report: fixed benchmark cases against analytic results (`docs/VALIDATION.md`)

#### Phase 7: optimisation and production
- Fast assemblies: components placed in parallel and reused while unchanged, instanced 3D drawing, a benchmark harness (`docs/BENCHMARKS.md`)
- Static studies, motion studies, interference checks and validation benchmarks run on a worker thread, with a progress line and "Cancel job"
- Exchange formats: glTF (`.glb`), 3MF and IFC4 export; STL and OBJ import as a mesh body; STEP export of cones and spheres
- Docked panels, light and dark themes with four accent colours, a settings file, command search (Ctrl+K), a rotate gizmo and gizmo drags of jointed components
- Plugin API: a Plugins menu, the MCP tool `apply_commands`, the `cad-plugin-api` crate with an example plugin, and `docs/PLUGINS.md`
- User guide (`docs/USER_GUIDE.md`) with screenshots made by `tests/mcp_docs_shots.sh`

### Changed
- Project file format 10 to 28; files from 0.1.0 still open
- The app runs analysis jobs on a worker thread instead of the UI thread
- CI runs `cargo clippy --workspace --all-targets -- -D warnings` before the tests
- README rewritten; `docs/DESIGN.md` and `docs/ROADMAP.md` updated for the whole roadmap

### Known limits
- What is not done is collected in "Beyond the roadmap" in [docs/ROADMAP.md](docs/ROADMAP.md); the accuracy labels and the benchmark cases are in [docs/VALIDATION.md](docs/VALIDATION.md).

## [0.1.0] - 2026-11-16

### Added
- Phase 1 core: workspace of `cad-geometry`, `cad-document`, `cad-renderer` and `cad-app`; shapes (line, circle, rectangle, box, cylinder, sphere); commands with atomic undo and redo; the `.cadproj` format; 2D and 3D viewports; selection, move, numeric edit and delete
- Phase 2 milestone 1: layers (colour, visibility, lock), snapping (endpoint, midpoint, centre, intersection), grid snap, Shift polar constraint, arcs and polylines, rotate, mirror, duplicate and scale
- Phase 2 milestone 2: trim, extend, offset, fillet and chamfer
- Phase 2 milestone 3: text and linear dimensions, units and decimals, SVG and DXF R12 export
- Phase 2 milestone 4: 15 constraint types on lines, circles and arcs with a Levenberg-Marquardt solver, degrees-of-freedom reporting and driving values
- Phase 2 milestone 5: layer line types and weights, ellipses, splines, polygons, perpendicular and tangent snap, typed coordinates, arrays
- Phase 2 milestone 6: hatch, leaders, radial and angular dimensions, DXF import with a skip report, benchmark baseline
- A first part of Phase 3: parameters with units and expressions, B-Rep kernel, extrude and revolve with booleans, feature history, sketches on planes and faces, fillet, chamfer, sweep, loft, shell, patterns, mirror, and STL and OBJ export
- MIT license, CI and release workflows

[0.2.0]: https://github.com/lucas-s-b/ProjectCAD/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/lucas-s-b/ProjectCAD/releases/tag/v0.1.0
